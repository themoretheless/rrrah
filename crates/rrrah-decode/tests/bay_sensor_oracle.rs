use rrrah_core::MemoryBudget;
use rrrah_decode::{BaySensorLayout, DecodeRequest, read_bay_sensor_with_budget};

#[test]
#[ignore = "requires external BAY source and independently decoded dcraw raw PGM"]
fn qv5700_active_sensor_matches_independent_document_output() {
    let source = std::env::var_os("RRRAH_BAY_SOURCE").expect("RRRAH_BAY_SOURCE");
    let oracle =
        std::fs::read(std::env::var_os("RRRAH_BAY_SENSOR_ORACLE").expect("RRRAH_BAY_SENSOR_ORACLE")).unwrap();
    let header = b"P5\n2576 1924\n65535\n";
    assert!(oracle.starts_with(header));
    assert_eq!(oracle.len(), header.len() + 2576 * 1924 * 2);
    let budget = MemoryBudget::new(32 * 1024 * 1024);
    let sensor =
        read_bay_sensor_with_budget(&DecodeRequest::new(source), BaySensorLayout::Qv5700, &budget).unwrap();
    assert_eq!(sensor.len(), 2585 * 1924);
    for (row, reference) in sensor
        .chunks_exact(2585)
        .zip(oracle[header.len()..].chunks_exact(2576 * 2))
    {
        for (actual, encoded) in row[..2576].iter().zip(reference.chunks_exact(2)) {
            assert_eq!(*actual, u16::from_be_bytes([encoded[0], encoded[1]]));
        }
    }
    drop(sensor);
    assert_eq!(budget.used(), 0);
    eprintln!(
        "QV5700: all 4956224 active sensor codes match independent document output; managed memory released"
    );
}
