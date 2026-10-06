//! Real process checks for quality settings, source opcodes and cache hits.
use std::{path::Path, process::Command};
fn dng(xtrans: bool, gain: bool) -> Vec<u8> {
    let mut b = vec![0_u8; 4096];
    b[..8].copy_from_slice(b"II*\0\x08\0\0\0");
    b.extend((0..144).flat_map(|_| 2048_u16.to_le_bytes()));
    let mut entries = Vec::<(u16, u16, u32, Vec<u8>)>::new();
    for (tag, value) in [
        (256, 12_u32),
        (257, 12),
        (273, 4096),
        (278, 12),
        (279, 288),
        (50717, 4095),
    ] {
        entries.push((tag, 4, 1, value.to_le_bytes().to_vec()));
    }
    for (tag, value) in [(258, 16_u16), (259, 1), (262, 32803), (277, 1), (274, 1)] {
        entries.push((tag, 3, 1, value.to_le_bytes().to_vec()));
    }
    entries.push((50706, 1, 4, vec![1, 4, 0, 0]));
    entries.push((50708, 2, 5, b"TEST\0".to_vec()));
    let side = if xtrans { 6_u16 } else { 2 };
    entries.push((33421, 3, 2, [side.to_le_bytes(), side.to_le_bytes()].concat()));
    let pattern = if xtrans {
        vec![
            1, 2, 1, 1, 0, 1, 0, 1, 0, 2, 1, 2, 1, 2, 1, 1, 0, 1, 1, 0, 1, 1, 2, 1, 2, 1, 2, 0, 1, 0, 1, 0,
            1, 1, 2, 1,
        ]
    } else {
        vec![0, 1, 1, 2]
    };
    entries.push((33422, 1, pattern.len() as u32, pattern));
    entries.push((
        50721,
        10,
        9,
        (0..9)
            .flat_map(|i| [i32::from(i % 4 == 0).to_le_bytes(), 1_i32.to_le_bytes()].concat())
            .collect(),
    ));
    entries.push((
        50728,
        5,
        3,
        [(1_u32, 2_u32), (1, 1), (2, 3)]
            .into_iter()
            .flat_map(|(n, d)| [n.to_le_bytes(), d.to_le_bytes()].concat())
            .collect(),
    ));
    if gain {
        // Independent DNG wire: count, id=GainMap, minVersion, flags, size.
        let mut params = Vec::new();
        for v in [0_u32, 0, 12, 12, 0, 1, 1, 1, 1, 1] {
            params.extend_from_slice(&v.to_be_bytes());
        }
        for v in [1.0_f64, 1.0, 0.0, 0.0] {
            params.extend_from_slice(&v.to_be_bytes());
        }
        params.extend_from_slice(&1_u32.to_be_bytes());
        params.extend_from_slice(&2.0_f32.to_be_bytes());
        let mut wire = Vec::new();
        for v in [1_u32, 9, 0x0103_0000, 0, params.len() as u32] {
            wire.extend_from_slice(&v.to_be_bytes());
        }
        wire.extend(params);
        entries.push((51009, 7, wire.len() as u32, wire));
    }
    entries.sort_by_key(|p| p.0);
    b[8..10].copy_from_slice(&(entries.len() as u16).to_le_bytes());
    for (i, (tag, kind, count, data)) in entries.into_iter().enumerate() {
        let at = 10 + i * 12;
        b[at..at + 2].copy_from_slice(&tag.to_le_bytes());
        b[at + 2..at + 4].copy_from_slice(&kind.to_le_bytes());
        b[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
        if data.len() <= 4 {
            b[at + 8..at + 8 + data.len()].copy_from_slice(&data);
        } else {
            let off = b.len() as u32;
            b[at + 8..at + 12].copy_from_slice(&off.to_le_bytes());
            b.extend(data);
        }
    }
    b
}
fn inspect(path: &Path, cache: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rrrah"))
        .arg("--inspect")
        .arg("--cache-dir")
        .arg(cache)
        .args(args)
        .arg(path)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}
#[test]
fn quality_settings_and_source_opcodes_survive_sensor_cache_hits() {
    let dir = tempfile::tempdir().unwrap();
    for (xtrans, gain, name) in [
        (false, false, "bayer.DNG"),
        (false, true, "gain.tif"),
        (true, false, "xtrans.dng"),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, dng(xtrans, gain)).unwrap();
        let cache = dir.path().join(format!("cache-{name}"));
        let first = inspect(&path, &cache, &["--raw-quality"]);
        assert!(first.contains("quality_raw: 12x12"));
        assert!(first.contains("cache_hit: false"));
        let hit = inspect(
            &path,
            &cache,
            &[
                "--raw-quality",
                "--no-highlight-recovery",
                "--tone-curve",
                "0:0,0.5:0.3,1:1",
            ],
        );
        assert!(hit.contains("cache_hit: true"));
        assert!(hit.contains("highlight_recovery=false"));
        assert!(hit.contains("display_curve_points=3"));
        let auto = inspect(&path, &cache, &[]);
        assert_eq!(auto.contains("quality_raw:"), xtrans || gain);
    }
}
#[test]
fn required_unknown_opcodes_and_bad_curves_fail_explicitly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.dng");
    let mut bytes = dng(false, true);
    let count = u16::from_le_bytes(bytes[8..10].try_into().unwrap()) as usize;
    for i in 0..count {
        let at = 10 + i * 12;
        if u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) == 51009 {
            let off = u32::from_le_bytes(bytes[at + 8..at + 12].try_into().unwrap()) as usize;
            bytes[off + 4..off + 8].copy_from_slice(&999_u32.to_be_bytes());
        }
    }
    std::fs::write(&path, bytes).unwrap();
    for (args, expected) in [
        (vec!["--inspect", "--no-cache"], "unsupported required opcode"),
        (
            vec!["--inspect", "--tone-curve", "0:0,0.5:0.8,1:0.2"],
            "invalid RAW development input: curve",
        ),
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_rrrah"))
            .args(args)
            .arg(&path)
            .output()
            .unwrap();
        assert!(!out.status.success());
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains(expected), "expected {expected:?}, got {error:?}");
    }
}
