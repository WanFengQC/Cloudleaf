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

#[test]
fn damaged_search_resource_does_not_block_literal_input_with_a_saved_scheme() {
    let directory =
        std::env::temp_dir().join(format!("cloudleaf-cli-profile-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(
        directory.join("input-settings.json"),
        r#"{"schema_version":1,"revision":7,"scheme":"abbreviated","fuzzy_pairs":[],"correction":false,"mixed_input":true,"page_size":3,"max_candidates":9}"#,
    )
    .unwrap();
    let returned = Command::new(env!("CARGO_BIN_EXE_cloudleaf-cli"))
        .arg("--settings-dir")
        .arg(&directory)
        .arg("--search-lexicon")
        .arg(directory.join("missing.tsv"))
        .arg("--search-sha")
        .arg("0".repeat(64))
        .args(["--replay", "YuN <Raw:@example.org>"])
        .output()
        .unwrap();
    std::fs::remove_file(directory.join("input-settings.json")).unwrap();
    std::fs::remove_dir(directory).unwrap();
    assert!(returned.status.success(), "{:?}", returned.stderr);
    assert!(
        String::from_utf8(returned.stdout)
            .unwrap()
            .contains("YuN@example.org")
    );
    assert!(
        String::from_utf8(returned.stderr)
            .unwrap()
            .contains("保存的输入方案尚未应用")
    );
}
