use std::path::Path;

use oneform::{cli_epf, cli_oneform, props};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let stem = args
        .first()
        .and_then(|a| Path::new(a).file_stem())
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let rest = &args[1..];
    let code = if stem == "epf-extract" {
        cli_epf::main_with_prog(rest, "epf-extract")
    } else if stem == "epf-pack" {
        cli_epf::pack_main(rest)
    } else if rest.first().map(String::as_str) == Some("epf") {
        if rest.get(1).map(String::as_str) == Some("--pack") {
            cli_epf::pack_main(&rest[2..])
        } else {
            cli_epf::main_with_prog(&rest[1..], "epf-extract")
        }
    } else if rest.first().map(String::as_str) == Some("props") {
        let (code, out, err) = props::run(&rest[1..]);
        print!("{out}");
        eprint!("{err}");
        code
    } else {
        cli_oneform::main(rest)
    };
    std::process::exit(code);
}
