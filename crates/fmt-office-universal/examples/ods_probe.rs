fn main() {
    let p = std::env::var("ODS_PROBE").unwrap_or("/tmp/opencode/sample.ods".into());
    let bytes = std::fs::read(p).unwrap();
    match viewit_fmt_office_universal::render(&bytes, "ods") {
        Ok(json) => println!("{}", json),
        Err(e) => println!("ERR {}", e),
    }
}
