use std::process::Command;

#[test]
fn replay_submits_explicit_literal_and_standard_candidates() {
    let returned = Command::new(env!("CARGO_BIN_EXE_cloudleaf-cli"))
        .args([
            "--replay",
            "nihao <Space> YuN <Raw:@example.org> <Raw:https://example.org/v2?q=1>",
        ])
        .output()
        .unwrap();
    assert!(returned.status.success(), "{:?}", returned.stderr);
    let output = String::from_utf8(returned.stdout).unwrap();
    assert!(output.contains("提交 #1：你好"));
    assert!(output.contains("提交 #2：YuN@example.org"));
    assert!(output.contains("提交 #3：https://example.org/v2?q=1"));
}
