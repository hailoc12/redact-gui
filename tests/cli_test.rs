//! Integration test: chạy binary thật trên file thật, zero dev-dependency.

use std::path::PathBuf;
use std::process::Command;

fn s(p: &std::path::Path) -> String {
    p.to_string_lossy().into_owned()
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_redact-tool"))
}

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("redact-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &PathBuf, content: &str) -> PathBuf {
    std::fs::write(path, content).unwrap();
    path.clone()
}

#[test]
fn redacts_text_file_to_sibling_output() {
    let dir = tmp_dir("basic");
    let input = write(
        &dir.join("baocao.txt"),
        "Người phụ trách: Nguyễn Văn A\nSĐT: 0901 234 567\nEmail: a.nguyen@coquan.gov.vn\nCCCD: 012345678901\n",
    );

    let out = bin().args([&s(&input)]).output().unwrap();
    assert!(
        out.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    let redacted = std::fs::read_to_string(dir.join("baocao.redacted.txt")).unwrap();
    assert!(!redacted.contains("0901"), "phone leak: {redacted}");
    assert!(!redacted.contains("a.nguyen@"), "email leak: {redacted}");
    assert!(!redacted.contains("012345678901"), "cccd leak: {redacted}");
    assert!(redacted.contains("[SDT_1]"));
    assert!(redacted.contains("[EMAIL_1]"));
    assert!(redacted.contains("[CCCD_1]"));
    // file gốc phải còn nguyên
    let orig = std::fs::read_to_string(&input).unwrap();
    assert!(orig.contains("0901 234 567"));
}

#[test]
fn dry_run_writes_nothing() {
    let dir = tmp_dir("dry");
    let input = write(&dir.join("a.txt"), "0901234567");

    let out = bin().args(["--dry-run", &s(&input)]).output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        1,
        "dry-run không được ghi file mới"
    );
}

#[test]
fn in_place_overwrites() {
    let dir = tmp_dir("inplace");
    let input = write(&dir.join("a.txt"), "0901234567");
    let out = bin().args(["--in-place", &s(&input)]).output().unwrap();
    assert!(out.status.success());
    let content = std::fs::read_to_string(&input).unwrap();
    assert_eq!(content.trim(), "[SDT_1]");
}

#[test]
fn pattern_subset_select() {
    let dir = tmp_dir("subset");
    let input = write(&dir.join("a.txt"), "0901234567 a@b.com");
    let out = bin()
        .args(["--patterns", "sdt", &s(&input)])
        .output()
        .unwrap();
    assert!(out.status.success());
    let content = std::fs::read_to_string(dir.join("a.redacted.txt")).unwrap();
    assert!(content.contains("[SDT_1]"));
    assert!(
        content.contains("a@b.com"),
        "email phải còn nguyên khi chỉ chọn sdt"
    );
}

#[test]
fn list_patterns_exits_zero() {
    let out = bin().arg("--list-patterns").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("cccd") && stdout.contains("tendn"));
}

#[test]
fn custom_rules_json() {
    let dir = tmp_dir("rules");
    let rules = write(
        &dir.join("rules.json"),
        r#"[{"id":"sohopdong","label":"HD","regex":"HD-\\d{4,}"}]"#,
    );
    let input = write(&dir.join("a.txt"), "Hợp đồng HD-2024-12345 ký ngày 5/5");
    let out = bin()
        .args(["--rules", &s(&rules), &s(&input)])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let content = std::fs::read_to_string(dir.join("a.redacted.txt")).unwrap();
    assert!(
        content.contains("[HD_1]"),
        "custom rule phải chạy: {content}"
    );
}

#[test]
fn map_file_contains_mapping() {
    let dir = tmp_dir("map");
    let input = write(&dir.join("a.txt"), "0901234567");
    let map = dir.join("map.json");
    let out = bin()
        .args(["--map", &s(&map), &s(&input)])
        .output()
        .unwrap();
    assert!(out.status.success());
    let mapping = std::fs::read_to_string(&map).unwrap();
    assert!(mapping.contains("\"original\": \"0901234567\""));
    assert!(mapping.contains("[SDT_1]"));
}

#[test]
fn json_output_stays_valid_json() {
    let dir = tmp_dir("json");
    let input = write(
        &dir.join("data.json"),
        r#"{"can_bo":"Nguyễn Văn A","sdt":"0901234567","mst":"0101234567"}"#,
    );
    let out = bin().arg(&input).output().unwrap();
    assert!(out.status.success());
    let content = std::fs::read_to_string(dir.join("data.redacted.json")).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&content).expect("output phải vẫn là JSON hợp lệ");
    assert_eq!(v["sdt"], "[SDT_1]");
    assert_eq!(v["mst"], "[MST_1]");
    assert_eq!(v["can_bo"], "Nguyễn Văn A");
}

#[test]
fn recursive_directory() {
    let dir = tmp_dir("recur");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    write(&dir.join("a.txt"), "0901234567");
    write(&dir.join("sub/b.txt"), "0901234567");

    let out = bin().args(["--recursive", &s(&dir)]).output().unwrap();
    assert!(out.status.success());
    assert!(dir.join("a.redacted.txt").exists());
    assert!(dir.join("sub/b.redacted.txt").exists());
}

#[test]
fn unredact_round_trip_cli() {
    let dir = tmp_dir("unredact");
    let input = write(&dir.join("a.txt"), "SĐT: 0901234567 và email a@b.com");
    let dict = dir.join("a.dict.json");

    let out = bin()
        .args(["--map", &s(&dict), &s(&input)])
        .output()
        .unwrap();
    assert!(out.status.success());

    let redacted_path = dir.join("a.redacted.txt");
    let out = bin()
        .args(["unredact", &s(&redacted_path), "--dict", &s(&dict)])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let restored = std::fs::read_to_string(dir.join("a.restored.txt")).unwrap();
    assert_eq!(restored.trim(), "SĐT: 0901234567 và email a@b.com");
}

#[test]
fn level_selects_pattern_subset() {
    let dir = tmp_dir("level");
    let input = write(&dir.join("a.txt"), "0901234567 012345678901 a@b.com");
    let out = bin().args(["--level", "1", &s(&input)]).output().unwrap();
    assert!(out.status.success());
    let content = std::fs::read_to_string(dir.join("a.redacted.txt")).unwrap();
    assert!(content.contains("[SDT_1]"));
    assert!(content.contains("[EMAIL_1]"));
    assert!(content.contains("012345678901"), "cấp 1 không redact CCCD");
}
