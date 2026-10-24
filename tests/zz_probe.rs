use oneform::brace::{parse_spanned, Item};
use oneform::form_patch::ControlIndex;
fn w(n: &oneform::brace::Node) -> String { let mut b = Vec::new(); oneform::donor::write_node(n, &mut b); String::from_utf8_lossy(&b).replace("\r\n", "").replace('\n', "") }
#[test]
fn probe() {
    let want = std::env::var("PROBE_TYPES").unwrap_or_default();
    let path: Vec<usize> = std::env::var("PROBE_PATH").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
    for name in ["DonorForm", "DonorFormAlt", "DonorFormV2"] {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forms").join(name).join("FormStream.bin");
        let s = parse_spanned(&std::fs::read(p).unwrap()).unwrap();
        let idx = ControlIndex::build(&s).unwrap();
        for e in &idx.entries {
            if !want.split(',').any(|t| t == e.type_name) { continue; }
            let mut n = e.info().unwrap().node;
            for &i in &path { n = n.children[i].as_node().unwrap(); }
            println!("### {name} {} {}", e.type_name, e.name);
            for (i, c) in n.children.iter().enumerate() {
                let t = match c { Item::Node(x) => w(x), Item::Atom(a) => format!("{:?}", a.value) };
                let t: String = t.chars().take(100).collect();
                println!("  [{i}] {t}");
            }
        }
    }
}
