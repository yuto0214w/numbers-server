use std::{
    env, fs,
    io::Write as _,
    path::PathBuf,
    process::{Command, Stdio},
};

const PROCEDURE_SOURCE: &'static str = "res/procedure.pu";

const PROCEDURE_FALLBACK: &'static str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="480" height="40">"#,
    r##"<rect width="100%" height="100%" fill="#000"/>"##,
    r##"<text x="12" y="25" fill="#fff" font-family="sans-serif" font-size="14">"##,
    "PlantUML がないため図を生成できませんでした",
    "</text></svg>",
);

fn render_svg(source: &[u8]) -> Option<Vec<u8>> {
    let mut child = Command::new("plantuml")
        .args(["-tsvg", "-pipe"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(source).ok()?;
    let output = child.wait_with_output().ok()?;
    output.status.success().then_some(output.stdout)
}

fn main() {
    println!("cargo:rerun-if-changed={}", PROCEDURE_SOURCE);
    let source = fs::read(PROCEDURE_SOURCE).unwrap();
    let svg = render_svg(&source).unwrap_or_else(|| {
        println!(
            "cargo:warning=could not render {} with PlantUML, embedding a placeholder",
            PROCEDURE_SOURCE
        );
        PROCEDURE_FALLBACK.as_bytes().to_vec()
    });
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out_dir.join("procedure.svg"), svg).unwrap();
}
