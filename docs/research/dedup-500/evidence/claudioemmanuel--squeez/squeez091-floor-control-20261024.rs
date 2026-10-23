use squeez::context::cache::SessionContext;
fn main() {
    let mut skill=SessionContext::default();let old=skill.skill_dedup_record(7);skill.dedup_floor_call=old;
    assert_eq!(skill.skill_dedup_lookup(7),None);let fresh=skill.skill_dedup_record(7);assert!(fresh>old);assert_eq!(skill.skill_dedup_lookup(7),None);
    let skill_back=SessionContext::from_json(&skill.to_json());assert_eq!(skill_back.skill_dedup_lookup(7),None);
    let mut image=SessionContext::default();let old=image.image_dedup_record(7);image.dedup_floor_call=old;
    assert_eq!(image.image_dedup_lookup(7),None);let fresh=image.image_dedup_record(7);assert!(fresh>old);assert_eq!(image.image_dedup_lookup(7),None);
    let mut read=SessionContext::default();let old=read.next_call_n();read.read_dedup_record("/owned/example",7,old);read.dedup_floor_call=old;
    assert_eq!(read.read_dedup_lookup("/owned/example",7),None);let fresh=read.next_call_n();read.read_dedup_record("/owned/example",7,fresh);assert_eq!(read.read_dedup_lookup("/owned/example",7),None);
    let mut control=SessionContext::default();control.record_call("old",7,10,1);control.dedup_floor_call=1;control.record_call("fresh",7,10,2);assert_eq!(control.lookup_recent(7,10).unwrap().call_n,2);
    println!(r#"{{"scope":"Unmodified public SessionContext methods, finite in-memory histories; no hook execution","old_call":1,"new_call":2,"floor":1,"skill_fresh_entry_missed":true,"image_fresh_entry_missed":true,"read_fresh_entry_missed":true,"skill_miss_survives_json_roundtrip":true,"windowed_lookup_finds_fresh_control":true}}"#);
}
