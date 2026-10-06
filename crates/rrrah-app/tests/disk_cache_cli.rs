//! Real-process RAW cache-policy regression; no window or GPU required.
use std::{
    path::{Path, PathBuf},
    process::Command,
};
fn inspect(root: &Path, name: &str, options: &[&str]) -> String {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests")
        .join(name);
    let output = Command::new(env!("CARGO_BIN_EXE_rrrah"))
        .arg("--inspect")
        .arg("--cache-dir")
        .arg(root)
        .args(options)
        .arg(fixture)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn count(root: &Path) -> usize {
    std::fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .flat_map(|entry| std::fs::read_dir(entry.path()).unwrap().filter_map(Result::ok))
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rrc"))
        .count()
}
#[test]
fn inspect_applies_count_ttl_and_zero_admission_without_losing_raw_output() {
    let root = tempfile::tempdir().unwrap();
    let options = ["--disk-cache-count", "1"];
    assert!(inspect(root.path(), "IMG_9043.CR3", &options).contains("cache_hit: false"));
    assert!(inspect(root.path(), "IMG_9043.CR3", &options).contains("cache_hit: true"));
    assert!(inspect(root.path(), "IMG_9074.CR3", &options).contains("cache_hit: false"));
    assert_eq!(count(root.path()), 1);
    assert!(inspect(root.path(), "IMG_9043.CR3", &options).contains("cache_hit: false"));
    assert_eq!(count(root.path()), 1);
    for options in [["--disk-cache-count", "0"], ["--disk-cache-mb", "0"]] {
        let disabled = tempfile::tempdir().unwrap();
        let output = inspect(disabled.path(), "IMG_9043.CR3", &options);
        assert!(output.contains("raw: 6188x4120"));
        assert!(output.contains("cache_hit: false"));
        assert_eq!(count(disabled.path()), 0);
    }
    let expired = tempfile::tempdir().unwrap();
    for _ in 0..2 {
        assert!(
            inspect(expired.path(), "IMG_9043.CR3", &["--disk-cache-ttl-secs", "0"])
                .contains("cache_hit: false")
        );
    }
}

#[test]
fn inspect_managed_budget_covers_decode_and_persistent_restore() {
    let root = tempfile::tempdir().unwrap();
    let decoded = inspect(root.path(), "IMG_9043.CR3", &["--managed-memory-mb", "96"]);
    assert!(decoded.contains("cache_hit: false"));
    assert!(decoded.contains("managed_memory: used=50989120"));
    let restored = inspect(root.path(), "IMG_9043.CR3", &["--managed-memory-mb", "64"]);
    assert!(restored.contains("cache_hit: true"));
    assert!(restored.contains("managed_memory: used=50989120, peak=50989120"));
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3");
    for options in [
        vec!["--managed-memory-mb", "64", "--no-cache"],
        vec!["--managed-memory-mb", "0"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_rrrah"))
            .arg("--inspect")
            .arg("--cache-dir")
            .arg(root.path())
            .args(options)
            .arg(&fixture)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("memory reservation"));
        assert_eq!(count(root.path()), 1);
    }
}
