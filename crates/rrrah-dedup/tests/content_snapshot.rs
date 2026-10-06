use rrrah_dedup::exact::{ContentSnapshot, SnapshotError};

#[test]
fn full_digest_rejects_equal_length_mutation_and_budget_excess() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data");
    std::fs::write(&path, b"first").unwrap();
    let snapshot = ContentSnapshot::read(&path, 5, || false).unwrap();
    snapshot.verify(|| false).unwrap();
    assert_eq!(snapshot.digest(), *blake3::hash(b"first").as_bytes());
    std::fs::write(&path, b"other").unwrap();
    assert!(matches!(snapshot.verify(|| false), Err(SnapshotError::Changed)));
    assert!(matches!(
        ContentSnapshot::read(&path, 4, || false),
        Err(SnapshotError::Policy)
    ));
    assert!(matches!(
        ContentSnapshot::read(&path, 5, || true),
        Err(SnapshotError::Cancelled)
    ));
}
