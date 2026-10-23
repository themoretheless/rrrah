use squeez::context::retrieve;
fn main() {
    let base=std::env::args_os().nth(1).expect("owned state directory");
    std::env::set_var("SQUEEZ_DIR", &base);
    let original="quasar galactic spectral calibration result\n".repeat(4);
    let key=retrieve::store(&original).expect("store");
    assert_eq!(retrieve::retrieve(&key),Some(original));
    assert_eq!(retrieve::search("quasar galactic spectral",5).len(),1);
    let blob=retrieve::blobs_dir().join(&key);
    std::fs::remove_file(&blob).unwrap();
    let missing=retrieve::search("quasar galactic spectral",5).len();
    assert_eq!(missing,1);assert!(retrieve::retrieve(&key).is_none());
    std::fs::write(&blob,"unrelated changed bytes").unwrap();
    let corrupt=retrieve::search("quasar galactic spectral",5).len();
    assert_eq!(corrupt,1);assert!(matches!(retrieve::retrieve_checked(&key),retrieve::RetrieveOutcome::Corrupted));
    println!(r#"{{"scope":"Full unmodified public store/search/retrieve API with owned directory and synthetic text", "intact_roundtrip":true,"missing_blob_still_searchable":true,"missing_retrieve_refused":true,"corrupt_blob_still_searchable":true,"corrupt_retrieve_refused":true,"interpretation":"Sidecar hits are candidates, not verified live blob contents; public retrieval checks integrity separately."}}"#);
}
