use squeez::context::cache::SessionContext;
fn main() {
    let mut ctx = SessionContext::default();
    assert!(ctx.note_subagent_files("first", &["/src/a;b.rs".into()]).is_empty());
    let missed = ctx.note_subagent_files("second", &["/src/a;b.rs".into()]);
    let invented = ctx.note_subagent_files("third", &["/src/a".into()]);
    assert!(missed.is_empty());
    assert_eq!(invented, vec!["/src/a"]);
    let mut restored = SessionContext::from_json(&ctx.to_json());
    assert!(restored.note_subagent_files("fourth", &["/src/a;b.rs".into()]).is_empty());
    let mut ordinary = SessionContext::default();
    ordinary.note_subagent_files("first", &["/src/ordinary.rs".into()]);
    assert_eq!(ordinary.note_subagent_files("second", &["/src/ordinary.rs".into()]), vec!["/src/ordinary.rs"]);
    println!(r#"{{"scope":"Unmodified pinned full-library public API; owned path strings only, no actual file or host integration", "semicolon_same_path_detected":false, "unseen_prefix_falsely_detected":true, "same_path_miss_survives_json_roundtrip":true, "ordinary_path_control_passed":true}}"#);
}
