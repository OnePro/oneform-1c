//! `oneform`: port of the Python Reference (`scripts/epf_extract`). See `spec.md` / `plan.md`.

#![allow(clippy::chunks_exact_to_as_chunks, clippy::manual_is_multiple_of)]
pub mod brace;
pub mod canonical;
pub mod cli_epf;
pub mod cli_oneform;
pub mod config;
pub mod container;
pub mod discovery;
pub mod donor;
pub mod encoding;
pub mod errors;
pub mod explore;
pub mod form_basic;
pub mod form_patch;
pub mod formbin;
pub mod manifest;
pub mod metadata;
pub mod metaref;
pub mod naming;
pub mod oof;
pub mod oof_props;
pub mod pack;
pub mod preview;
pub mod props;
pub mod report;
pub mod stage;
pub mod templates;
pub mod transaction;
