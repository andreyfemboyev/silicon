use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Output},
};
fn command(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_silicon"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}
fn json(dir: &Path, args: &[&str]) -> Value {
    let out = command(dir, args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn new_project_assemble_run_export_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    json(dir, &["--json", "init", "work.json"]);
    assert!(!command(dir, &["init", "work.json"]).status.success());
    assert_eq!(
        json(dir, &["--json", "validate", "work.json"])["valid"],
        true
    );
    assert_eq!(
        json(dir, &["arch", "inspect", "work.json", "--json"])["name"],
        "MICRO-8"
    );
    let result = json(
        dir,
        &[
            "--json",
            "assemble",
            "work.json",
            "-o",
            "program.bin",
            "--symbols",
            "symbols.json",
        ],
    );
    assert_eq!(result["symbols"]["loop"], 8);
    assert!(std::fs::read(dir.join("program.bin")).unwrap().len() > 8);
    let lines = json(dir, &["--json", "disassemble", "work.json", "program.bin"]);
    assert_eq!(lines[0], "SET r0, 0");
    let report = json(dir, &["--json", "run", "work.json", "--memory", "240"]);
    assert_eq!(report["stop"], "halted");
    assert_eq!(report["memory"][0], 21);
    let reports = json(dir, &["--json", "trace", "work.json", "--history", "3"]);
    assert_eq!(reports["trace"].as_array().unwrap().len(), 3);
    let wave = json(
        dir,
        &[
            "--json",
            "simulate",
            "work.json",
            "--ticks",
            "12",
            "--vcd",
            "signals.vcd",
        ],
    );
    assert!(wave["events"].as_array().unwrap().len() > 6);
    assert!(std::fs::read_to_string(dir.join("signals.vcd"))
        .unwrap()
        .contains("#12"));
    assert_eq!(json(dir, &["--json", "test"]).as_array().unwrap().len(), 3);
}
#[test]
fn malformed_input_produces_error_json_and_no_output_file() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    std::fs::write(dir.join("invalid.json"), b"{}").unwrap();
    let out = command(
        dir,
        &["--json", "assemble", "invalid.json", "-o", "bad.bin"],
    );
    assert!(!out.status.success());
    let diagnostic: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert!(diagnostic["error"].is_string());
    assert!(!dir.join("bad.bin").exists());
}
