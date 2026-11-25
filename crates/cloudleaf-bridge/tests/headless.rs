use serde_json::Value;
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

#[test]
fn headless_preserves_normal_commit_and_keeps_text_private_after_a_model_failure() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut child = Command::new(env!("CARGO_BIN_EXE_cloudleaf-bridge-headless"))
        .arg(fixtures.join("lexicon.tsv"))
        .arg("50b020cbe8813bf783a1ac66731e93771439ddff9a6b9a369dde58332c704041")
        .arg(fixtures.join("missing-catalog.json"))
        .arg("0".repeat(64))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let actions = [
        r#"{"op":"activate"}"#,
        r#"{"op":"type","text":"nihao"}"#,
        r#"{"op":"candidate"}"#,
        r#"{"op":"wait"}"#,
        r#"{"op":"space"}"#,
        r#"{"op":"type","text":"nihao"}"#,
        r#"{"op":"enter"}"#,
        r#"{"op":"type","text":"nihao1"}"#,
    ];
    let mut input = child.stdin.take().unwrap();
    for action in actions {
        writeln!(input, "{action}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let records: Vec<Value> = std::str::from_utf8(&output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), actions.len());
    assert_eq!(records[3]["translation_failure"], "ModelNotFound");
    assert_eq!(records[3]["preedit_bytes"], 5);
    assert_eq!(records[4]["document_bytes"], 6);
    assert_eq!(records[6]["document_bytes"], 12);
    assert_eq!(records[7]["document_bytes"], 18);
    for record in records {
        assert!(record["error"].is_null());
        assert!(record["original"].is_null());
        assert!(record["translation"].is_null());
        assert!(record["document"].is_null());
    }
}
