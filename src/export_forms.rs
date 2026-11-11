//! Plan of `oneform export-forms`: the `Form.xml` and `FormModule.bsl` of every ordinary form of a configuration dump,
//! in memory, one way (a view of the dump for version control; nothing here can be packed back).
//!
//! The planning reads the dump and writes nothing. The path of a form in the plan is the path of the form in the dump
//! without `Ext/Form.bin`: `<Type>/<Object>/Forms/<Form>/` and `CommonForms/<Name>/`, never an alias and never `src/`.
//! The content is what `extract` writes (the same [`project_form`]): the built-in decoder, the default limits, no
//! environment, no `.oneform.json`.

use std::collections::BTreeMap;
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use rayon::prelude::*;

use crate::container::Limits;
use crate::errors::{ExtractionError, IO_ERROR};
use crate::formbin::{
    discover_config_objects, form_bin_rel, object_dir, project_form, read_object_meta, ConfigObject,
};
use crate::metaref::ConfigTypeLookup;

mod apply;
mod cli;
pub use cli::run_cli;
pub use apply::{
    apply_export, check_export, check_out_location, ApplyReport, Difference, DifferenceKind, ExportError, ExportRefusal, MARKER_FILE,
};

pub const FORM_XML_FILE: &str = "Form.xml";
pub const FORM_MODULE_FILE: &str = "FormModule.bsl";
/// Failure code of a form whose projection panicked.
pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";

/// An ordinary form of the dump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportForm {
    /// Output directory, `/` separated, relative to the output root: the dump directory of the form.
    pub dir: String,
    /// Directory name of the form in the dump.
    pub name: String,
    /// Object directory relative to the dump (see [`ConfigObject::rel`]).
    pub object: String,
}

/// A form that has no `Form.xml` projection: its `Form.xml` is planned with zero bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithoutProjection {
    /// Form directory (see [`ExportForm::dir`]).
    pub path: String,
    /// `format-8.0` or `metadata-layout` (the codes of `oneform validate`).
    pub reason: String,
}

/// A form that could not be read at all: it is not in the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFailure {
    pub path: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportReport {
    /// Sorted by path.
    pub without_projection: Vec<WithoutProjection>,
    /// Sorted by path.
    pub failures: Vec<ExportFailure>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportPlan {
    /// Relative path (`/`) -> bytes: `<dir>/Form.xml` and `<dir>/FormModule.bsl` of every readable form.
    pub tree: BTreeMap<String, Vec<u8>>,
    pub report: ExportReport,
    /// All forms found in the dump, failed ones included.
    pub form_count: usize,
}

/// Output directory of the form `name` of `obj`: the dump directory names, no `src`, no alias.
pub fn form_dir(obj: &ConfigObject, name: &str) -> String {
    let rel = if obj.rel == "." { "" } else { obj.rel.as_str() };
    match (obj.common, rel.is_empty()) {
        (true, false) => rel.to_string(),
        (true, true) => name.to_string(),
        (false, false) => format!("{rel}/Forms/{name}"),
        (false, true) => format!("Forms/{name}"),
    }
}

/// The ordinary forms of the dump `root`, sorted by output directory.
pub fn discover_forms(root: &Path) -> Vec<ExportForm> {
    forms_of(&discover_config_objects(root))
}

fn forms_of(objects: &[ConfigObject]) -> Vec<ExportForm> {
    let mut forms: Vec<ExportForm> = objects
        .iter()
        .flat_map(|o| o.forms.iter().map(|n| ExportForm { dir: form_dir(o, n), name: n.clone(), object: o.rel.clone() }))
        .collect();
    forms.sort_by(|a, b| a.dir.cmp(&b.dir));
    forms
}

enum Outcome {
    Planned { dir: String, xml: Option<Vec<u8>>, module: Vec<u8>, reason: Option<&'static str> },
    Failed { dir: String, error: ExtractionError },
}

fn plan_object(root: &Path, obj: &ConfigObject, types: &ConfigTypeLookup, limits: &Limits) -> Vec<Outcome> {
    let meta = read_object_meta(root, obj);
    obj.forms
        .iter()
        .map(|name| {
            let dir = form_dir(obj, name);
            let run = catch_unwind(AssertUnwindSafe(|| -> Result<_, ExtractionError> {
                let raw = fs::read(object_dir(root, obj).join(form_bin_rel(obj, name)))
                    .map_err(|e| ExtractionError::new(IO_ERROR, e.to_string()))?;
                project_form(root, obj, name, &raw, &meta, types, limits, name, None)
            }));
            match run {
                Ok(Ok(p)) => Outcome::Planned { dir, xml: p.form_xml, module: p.module, reason: p.reason },
                Ok(Err(error)) => Outcome::Failed { dir, error },
                Err(_) => Outcome::Failed { dir, error: ExtractionError::new(INTERNAL_ERROR, "panic while projecting the form") },
            }
        })
        .collect()
}

/// The tree of `Form.xml` and `FormModule.bsl` of every ordinary form of the dump `root`, and the report.
///
/// Objects are processed by `workers` threads (64 MiB stacks; one thread when 1 or when the pool cannot be built). The
/// result does not depend on `workers`. A form that fails is reported and left out of the tree; the others go on.
pub fn plan_export(root: &Path, workers: usize) -> ExportPlan {
    let objects = discover_config_objects(root);
    let types = ConfigTypeLookup::from_config_dir(root);
    let limits = Limits::default();
    let run = |o: &ConfigObject| plan_object(root, o, &types, &limits);
    let per_object: Vec<Vec<Outcome>> = if workers > 1 && objects.len() > 1 {
        match rayon::ThreadPoolBuilder::new().num_threads(workers.min(objects.len())).stack_size(64 << 20).build() {
            Ok(pool) => pool.install(|| objects.par_iter().map(run).collect()),
            Err(_) => objects.iter().map(run).collect(),
        }
    } else {
        objects.iter().map(run).collect()
    };
    let mut plan = ExportPlan::default();
    for outcome in per_object.into_iter().flatten() {
        plan.form_count += 1;
        match outcome {
            Outcome::Planned { dir, xml, module, reason } => {
                if let Some(reason) = reason {
                    plan.report.without_projection.push(WithoutProjection { path: dir.clone(), reason: reason.to_string() });
                }
                plan.tree.insert(format!("{dir}/{FORM_XML_FILE}"), xml.unwrap_or_default());
                plan.tree.insert(format!("{dir}/{FORM_MODULE_FILE}"), module);
            }
            Outcome::Failed { dir, error } => {
                plan.report.failures.push(ExportFailure { path: dir, code: error.code.to_string(), message: error.message })
            }
        }
    }
    plan.report.without_projection.sort_by(|a, b| a.path.cmp(&b.path));
    plan.report.failures.sort_by(|a, b| a.path.cmp(&b.path));
    plan
}
