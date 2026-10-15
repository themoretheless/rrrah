//! Bounded CIFF directory foundation. No CRW sensor or color decoding yet.
//! Records borrow the source; neither payloads nor directory entries are copied.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy)]
pub(crate) struct Directory<'a> {
    block: &'a [u8],
    table: usize,
    count: usize,
    little: bool,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct Record<'a> {
    pub tag: u16,
    pub payload: &'a [u8],
}
impl<'a> Directory<'a> {
    pub fn file(bytes: &'a [u8]) -> Result<Self, &'static str> {
        let little = match bytes.get(..2) {
            Some(b"II") => true,
            Some(b"MM") => false,
            _ => return Err("invalid CIFF byte order"),
        };
        if bytes.get(6..14) != Some(b"HEAPCCDR") {
            return Err("invalid CIFF signature");
        }
        let offset = read32(bytes.get(2..6).ok_or("short CIFF header")?, little) as usize;
        if offset < 14 {
            return Err("CIFF heap overlaps header");
        }
        Self::block(bytes.get(offset..).ok_or("CIFF heap outside source")?, little)
    }
    fn block(block: &'a [u8], little: bool) -> Result<Self, &'static str> {
        let end = block.len().checked_sub(4).ok_or("short CIFF block")?;
        let table = read32(&block[end..], little) as usize;
        let count_end = table.checked_add(2).ok_or("CIFF table overflow")?;
        if count_end > end {
            return Err("CIFF table outside block");
        }
        let count = usize::from(read16(&block[table..count_end], little));
        let records_end = count
            .checked_mul(10)
            .and_then(|n| count_end.checked_add(n))
            .ok_or("CIFF record count overflow")?;
        if records_end > end {
            return Err("CIFF records outside block");
        }
        let directory = Self {
            block,
            table,
            count,
            little,
        };
        for index in 0..count {
            directory.record(index)?;
        }
        Ok(directory)
    }
    /// Bounded tree lookup. A duplicate tag is ambiguous and must not silently
    /// select a camera geometry, decoder table or sensor payload.
    pub fn unique(&self, tag: u16, cancelled: &dyn Fn() -> bool) -> Result<Record<'a>, &'static str> {
        fn visit<'a>(
            directory: Directory<'a>,
            tag: u16,
            depth: usize,
            remaining: &mut usize,
            found: &mut Option<Record<'a>>,
            cancelled: &dyn Fn() -> bool,
        ) -> Result<(), &'static str> {
            if depth > 16 {
                return Err("CIFF nesting limit exceeded");
            }
            for index in 0..directory.len() {
                if cancelled() {
                    return Err("CIFF lookup cancelled");
                }
                *remaining = remaining.checked_sub(1).ok_or("CIFF record limit exceeded")?;
                let record = directory.record(index)?;
                if record.tag == tag {
                    if found.is_some() {
                        return Err("duplicate CIFF record");
                    }
                    *found = Some(record);
                }
                if record.tag & 0x4000 == 0 && matches!(record.tag & 0x3800, 0x2800 | 0x3000) {
                    visit(
                        directory.child(index)?,
                        tag,
                        depth + 1,
                        remaining,
                        found,
                        cancelled,
                    )?;
                }
            }
            Ok(())
        }
        let mut found = None;
        visit(*self, tag, 0, &mut 4096, &mut found, cancelled)?;
        found.ok_or("missing CIFF record")
    }

    pub fn len(&self) -> usize {
        self.count
    }
    pub fn record(&self, index: usize) -> Result<Record<'a>, &'static str> {
        if index >= self.count {
            return Err("CIFF record index outside table");
        }
        let start = self.table + 2 + index * 10;
        let entry = &self.block[start..start + 10];
        let tag = read16(entry, self.little);
        let payload = if tag & 0x4000 != 0 {
            &entry[2..10]
        } else {
            let length = read32(&entry[2..6], self.little) as usize;
            let offset = read32(&entry[6..10], self.little) as usize;
            let end = offset.checked_add(length).ok_or("CIFF payload overflow")?;
            if end > self.table {
                return Err("CIFF payload overlaps directory");
            }
            self.block.get(offset..end).ok_or("CIFF payload outside block")?
        };
        Ok(Record { tag, payload })
    }
    pub fn child(&self, index: usize) -> Result<Self, &'static str> {
        let record = self.record(index)?;
        if record.tag & 0x4000 != 0 || !matches!(record.tag & 0x3800, 0x2800 | 0x3000) {
            return Err("CIFF record is not a nested directory");
        }
        Self::block(record.payload, self.little)
    }
}
fn read16(bytes: &[u8], little: bool) -> u16 {
    let value = [bytes[0], bytes[1]];
    if little {
        u16::from_le_bytes(value)
    } else {
        u16::from_be_bytes(value)
    }
}
fn read32(bytes: &[u8], little: bool) -> u32 {
    let value = [bytes[0], bytes[1], bytes[2], bytes[3]];
    if little {
        u32::from_le_bytes(value)
    } else {
        u32::from_be_bytes(value)
    }
}

/// Assemble the independently qualified 10D Auto-WB metadata subset.
/// Unknown orientation, geometry or camera identity is an explicit error.
pub(crate) fn eos_10d_metadata(
    source: &[u8],
    pixels: &[u16],
    cancelled: &dyn Fn() -> bool,
) -> Result<rrrah_core::RawMetadata, crate::DecodeError> {
    use rrrah_core::{
        CfaColor, CfaPattern, LevelGrid, Orientation, Photometric, RawMetadata, Rect, WhiteLevel,
    };
    let error = |message: &str| crate::DecodeError::NativeCamera {
        format: "CRW",
        message: message.into(),
    };
    let gains = eos_10d_preflight(source, cancelled)?;
    let black = eos_10d_black(pixels, cancelled)?;
    let area = Rect::new(64, 12, 3088, 2056);
    let metadata = RawMetadata {
        make: "Canon".into(),
        model: "Canon EOS 10D".into(),
        width: 3152,
        height: 2068,
        components_per_pixel: 1,
        bits_per_sample: 12,
        photometric: Photometric::Cfa,
        cfa: Some(CfaPattern {
            width: 2,
            height: 2,
            cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
        }),
        black_level: LevelGrid {
            width: 2,
            height: 2,
            components: 1,
            values: black.map(f32::from).to_vec(),
        },
        white_level: WhiteLevel(vec![4000.0]),
        white_balance: gains.map(|v| f32::from(v) / f32::from(gains[1])),
        xyz_to_camera: eos_10d_xyz_to_camera(),
        active_area: Some(area),
        crop_area: Some(area),
        orientation: match read32(
            &Directory::file(source)
                .map_err(error)?
                .unique(0x1810, cancelled)
                .map_err(error)?
                .payload[12..],
            true,
        ) {
            0 => Orientation::Normal,
            90 => Orientation::Rotate90,
            180 => Orientation::Rotate180,
            270 => Orientation::Rotate270,
            _ => return Err(error("unsupported 10D orientation")),
        },
    };
    metadata.validate().map_err(|_| error("invalid 10D metadata"))?;
    Ok(metadata)
}

/// Metadata admission before allocating or decoding the sensor.
pub(crate) fn eos_10d_preflight(
    source: &[u8],
    cancelled: &dyn Fn() -> bool,
) -> Result<[u16; 4], crate::DecodeError> {
    let error = |message: &str| crate::DecodeError::NativeCamera {
        format: "CRW",
        message: message.into(),
    };
    if cancelled() {
        return Err(crate::DecodeError::Cancelled);
    }
    if source.get(..6) != Some(&b"II\x1a\0\0\0"[..]) {
        return Err(error("unsupported 10D header"));
    }
    let directory = Directory::file(source).map_err(error)?;
    let lookup = |tag| {
        directory.unique(tag, cancelled).map_err(|reason| {
            if cancelled() {
                crate::DecodeError::Cancelled
            } else {
                error(reason)
            }
        })
    };
    if !lookup(0x080a)?.payload.starts_with(b"Canon\0Canon EOS 10D\0") {
        return Err(error("unsupported CRW color camera"));
    }
    let geometry = lookup(0x1031)?.payload;
    if geometry.len() != 34
        || read16(geometry, true) != 34
        || read16(&geometry[2..], true) != 3152
        || read16(&geometry[4..], true) != 2068
    {
        return Err(error("unsupported 10D geometry"));
    }
    let info = lookup(0x1810)?.payload;
    if info.len() != 28 || !matches!(read32(&info[12..], true), 0 | 90 | 180 | 270) {
        return Err(error("unsupported 10D orientation"));
    }
    let gains = eos_10d_wb(lookup(0x10a9)?.payload, lookup(0x102a)?.payload).map_err(error)?;
    Ok(gains)
}

/// 10D optical-black means in full-sensor RGGB phase order. The qualified
/// masked region excludes the two edge columns and initial 16 columns.
pub(crate) fn eos_10d_black(
    pixels: &[u16],
    cancelled: &dyn Fn() -> bool,
) -> Result<[u16; 4], crate::DecodeError> {
    let error = |message: &str| crate::DecodeError::NativeCamera {
        format: "CRW",
        message: message.into(),
    };
    if pixels.len() != 3152 * 2068 {
        return Err(error("unsupported 10D sensor dimensions"));
    }
    let mut sums = [0u64; 4];
    let mut counts = [0u64; 4];
    for row in 12..2068 {
        if cancelled() {
            return Err(crate::DecodeError::Cancelled);
        }
        for column in 18..62 {
            let value = pixels[row * 3152 + column];
            if value > 4095 {
                return Err(error("invalid 10D optical-black sample"));
            }
            let phase = (row % 2) * 2 + column % 2;
            sums[phase] += u64::from(value);
            counts[phase] += 1;
        }
    }
    let mut result = [0; 4];
    for phase in 0..4 {
        result[phase] =
            u16::try_from(sums[phase] / counts[phase]).map_err(|_| error("black level overflow"))?;
        if result[phase] == 0 {
            return Err(error("missing 10D optical-black calibration"));
        }
    }
    Ok(result)
}

/// Pinned independent LibRaw 0.22.2 EOS 10D calibration for the new CRW path.
/// This is separate from the older general camera catalog profile.
pub(crate) fn eos_10d_xyz_to_camera() -> [[f32; 3]; 4] {
    [
        [0.8250, -0.2044, -0.1127],
        [-0.8092, 1.5606, 0.2664],
        [-0.2893, 0.3453, 0.8348],
        [0.0; 3],
    ]
}

/// Qualified 82-byte 10D WB layout. Modes without an independent valid
/// reference remain refused rather than guessing coefficients.
pub(crate) fn eos_10d_wb(color: &[u8], shot: &[u8]) -> Result<[u16; 4], &'static str> {
    if color.len() != 82 || read16(color, true) != 82 || shot.len() < 16 {
        return Err("unsupported 10D color/shot layout");
    }
    if usize::from(read16(shot, true)) != shot.len() {
        return Err("invalid 10D shot length");
    }
    // Independently qualified mode-to-record mapping for this 82-byte layout.
    // Modes 4/7 have unavailable independent coefficients and remain refused.
    let mode = read16(&shot[14..], true);
    let row = match mode {
        0 => 0,
        1 => 1,
        2 => 3,
        3 => 4,
        5 => 6,
        6 => 7,
        8 => 2,
        9 => 8,
        _ => return Err("10D WB mode not qualified"),
    };
    let at = 2 + row * 8;
    let gains = [
        read16(&color[at..], true),
        read16(&color[at + 2..], true),
        read16(&color[at + 6..], true),
        read16(&color[at + 4..], true),
    ];
    if gains.contains(&0) {
        return Err("missing 10D WB coefficient");
    }
    Ok(gains)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(little: bool, nested: bool) -> Vec<u8> {
        let word = |v: u16| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        let dword = |v: u32| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut payload = b"Canon\0D30\0".to_vec();
        let mut block = payload.clone();
        block.extend(word(1));
        block.extend(word(0x080a));
        block.extend(dword(payload.len() as u32));
        block.extend(dword(0));
        block.extend(dword(payload.len() as u32));
        if nested {
            payload = block;
            block = payload.clone();
            block.extend(word(1));
            block.extend(word(0x300a));
            block.extend(dword(payload.len() as u32));
            block.extend(dword(0));
            block.extend(dword(payload.len() as u32));
        }
        let mut file = if little { b"II".to_vec() } else { b"MM".to_vec() };
        file.extend(dword(14));
        file.extend(b"HEAPCCDR");
        file.extend(block);
        file
    }
    #[test]
    fn both_orders_nested_payloads_and_all_prefixes() {
        for little in [false, true] {
            let bytes = fixture(little, true);
            let root = Directory::file(&bytes).unwrap();
            assert_eq!(root.len(), 1);
            let child = root.child(0).unwrap();
            assert_eq!(child.record(0).unwrap().payload, b"Canon\0D30\0");
            assert!(child.child(0).is_err());
            assert!(root.record(1).is_err());
            for end in 0..bytes.len() {
                // CIFF has no total-length field: a prefix can itself end in a valid
                // directory footer. Every prefix must be safe to inspect.
                if let Ok(directory) = Directory::file(&bytes[..end]) {
                    for index in 0..directory.len() {
                        directory.record(index).unwrap();
                    }
                }
            }
        }
    }
    #[test]
    #[ignore = "requires pinned CC0 EOS D30 CRW object 1307"]
    fn real_d30_directory_tree_and_sensor_metadata() {
        let bytes = std::fs::read(std::env::var("RRRAH_CRW_SOURCE").unwrap()).unwrap();
        assert_eq!(bytes.len(), 2_993_260);
        let root = Directory::file(&bytes).unwrap();
        assert_eq!(root.record(0).unwrap().tag, 0x2005);
        assert_eq!(root.record(0).unwrap().payload.len(), 2_824_648);
        let mut directories = 0;
        let mut records = 0;
        let mut identity = None;
        let mut image_geometry = None;
        let mut sensor_geometry = None;
        let mut decoder_table = None;
        let mut stack = vec![(root, 0usize)];
        while let Some((directory, depth)) = stack.pop() {
            assert!(depth <= 8);
            directories += 1;
            for index in 0..directory.len() {
                records += 1;
                let record = directory.record(index).unwrap();
                match record.tag {
                    0x080a => identity = Some(record.payload),
                    0x1810 => {
                        image_geometry =
                            Some((read32(record.payload, true), read32(&record.payload[4..], true)))
                    }
                    0x1031 => {
                        sensor_geometry = Some((
                            read16(&record.payload[2..], true),
                            read16(&record.payload[4..], true),
                        ))
                    }
                    0x1835 => decoder_table = Some(read32(record.payload, true)),
                    _ => (),
                }
                if record.tag & 0x4000 == 0 && matches!(record.tag & 0x3800, 0x2800 | 0x3000) {
                    stack.push((directory.child(index).unwrap(), depth + 1));
                }
            }
        }
        assert_eq!((directories, records), (7, 39));
        assert!(identity.unwrap().starts_with(b"Canon\0Canon EOS D30\0"));
        assert_eq!(image_geometry, Some((2160, 1440)));
        assert_eq!(sensor_geometry, Some((2224, 1456)));
        assert_eq!(decoder_table, Some(1));
    }

    #[test]
    fn ten_d_calibration_transform_matches_independent_oracle() {
        let matrix = eos_10d_xyz_to_camera();
        let actual = rrrah_core::camera_to_linear_srgb(matrix).unwrap();
        let expected = [
            [1.555702448f64, -0.5919526815, 0.03625022247],
            [-0.02331979759, 1.380087614, -0.3567678034],
            [0.06781055033, -0.4607699811, 1.392959476],
        ];
        for (a, b) in actual.into_iter().flatten().zip(expected.into_iter().flatten()) {
            assert!((f64::from(a) - b).abs() < 1e-6, "{a} vs {b}");
        }
    }

    #[test]
    fn lookup_caps_admit_boundary_and_refuse_next_level_or_record() {
        fn file(block: Vec<u8>) -> Vec<u8> {
            let mut source = b"II".to_vec();
            source.extend(14u32.to_le_bytes());
            source.extend(b"HEAPCCDR");
            source.extend(block);
            source
        }
        for count in [4096u16, 4097] {
            let mut block = count.to_le_bytes().to_vec();
            for _ in 0..count {
                block.extend(0x580bu16.to_le_bytes());
                block.extend([0; 8]);
            }
            block.extend(0u32.to_le_bytes());
            let source = file(block);
            let expected = if count == 4096 {
                "missing CIFF record"
            } else {
                "CIFF record limit exceeded"
            };
            assert_eq!(
                Directory::file(&source)
                    .unwrap()
                    .unique(0xffff, &|| false)
                    .unwrap_err(),
                expected
            );
        }
        for depth in [16, 17] {
            let mut block = vec![0; 6]; // Empty directory: count zero, footer zero.
            for _ in 0..depth {
                let payload_length = block.len() as u32;
                block.extend(1u16.to_le_bytes());
                block.extend(0x300au16.to_le_bytes());
                block.extend(payload_length.to_le_bytes());
                block.extend(0u32.to_le_bytes());
                block.extend(payload_length.to_le_bytes());
            }
            let source = file(block);
            let expected = if depth == 16 {
                "missing CIFF record"
            } else {
                "CIFF nesting limit exceeded"
            };
            assert_eq!(
                Directory::file(&source)
                    .unwrap()
                    .unique(0xffff, &|| false)
                    .unwrap_err(),
                expected
            );
        }
    }

    #[test]
    fn ten_d_wb_layout_refuses_missing_coefficients_and_unknown_presets() {
        let mut color = [0u8; 82];
        color[..2].copy_from_slice(&82u16.to_le_bytes());
        for (index, value) in [1742u16, 832, 832, 1241].into_iter().enumerate() {
            color[2 + index * 2..4 + index * 2].copy_from_slice(&value.to_le_bytes());
        }
        let mut shot = [0u8; 66];
        shot[..2].copy_from_slice(&66u16.to_le_bytes());
        assert_eq!(eos_10d_wb(&color, &shot), Ok([1742, 832, 1241, 832]));
        for end in 0..color.len() {
            assert!(eos_10d_wb(&color[..end], &shot).is_err());
        }
        for index in 0..4 {
            let mut bad = color;
            bad[2 + index * 2..4 + index * 2].fill(0);
            assert!(eos_10d_wb(&bad, &shot).is_err());
        }
        shot[14] = 10;
        assert!(eos_10d_wb(&color, &shot).is_err());
    }
    #[test]
    #[ignore = "requires pinned CC0 EOS 10D CRW object 2027"]
    fn real_10d_auto_white_balance_matches_independent_oracle() {
        let source = std::fs::read(std::env::var("RRRAH_CRW_SOURCE").unwrap()).unwrap();
        let directory = Directory::file(&source).unwrap();
        assert!(
            directory
                .unique(0x080a, &|| false)
                .unwrap()
                .payload
                .starts_with(b"Canon\0Canon EOS 10D\0")
        );
        let color = directory.unique(0x10a9, &|| false).unwrap().payload;
        let shot = directory.unique(0x102a, &|| false).unwrap().payload;
        assert_eq!(eos_10d_wb(color, shot), Ok([1742, 832, 1241, 832]));
        let expected = [
            Some([1742, 832, 1241, 832]),
            Some([1797, 832, 1002, 832]),
            Some([1972, 832, 921, 832]),
            Some([1382, 970, 1779, 970]),
            None,
            Some([1999, 832, 920, 832]),
            Some([1797, 832, 1002, 832]),
            None,
            Some([2140, 832, 854, 832]),
            Some([1801, 832, 1001, 832]),
        ];
        for (mode, reference) in expected.into_iter().enumerate() {
            let mut modified = shot.to_vec();
            modified[14..16].copy_from_slice(&(mode as u16).to_le_bytes());
            assert_eq!(eos_10d_wb(color, &modified).ok(), reference);
        }
    }

    #[test]
    fn unique_lookup_is_cancellable_and_rejects_duplicates() {
        let source = fixture(true, true);
        let directory = Directory::file(&source).unwrap();
        assert_eq!(
            directory.unique(0x080a, &|| false).unwrap().payload,
            b"Canon\0D30\0"
        );
        assert_eq!(
            directory.unique(0x080a, &|| true).unwrap_err(),
            "CIFF lookup cancelled"
        );
        assert_eq!(
            directory.unique(0xffff, &|| false).unwrap_err(),
            "missing CIFF record"
        );
        let mut source = fixture(true, false);
        let footer = source.split_off(source.len() - 4);
        let entry = source[26..36].to_vec();
        source.extend(entry);
        source.extend(footer);
        source[24..26].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            Directory::file(&source)
                .unwrap()
                .unique(0x080a, &|| false)
                .unwrap_err(),
            "duplicate CIFF record"
        );
    }

    #[test]
    fn malicious_offsets_counts_and_inline_records() {
        let original = fixture(true, false);
        for value in [0, 13, u32::MAX] {
            let mut bytes = original.clone();
            bytes[2..6].copy_from_slice(&value.to_le_bytes());
            assert!(Directory::file(&bytes).is_err());
        }
        let mut bytes = original.clone();
        let end = bytes.len();
        bytes[end - 4..].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Directory::file(&bytes).is_err());
        let table = 24;
        let mut bytes = original.clone();
        bytes[table..table + 2].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(Directory::file(&bytes).is_err());
        let mut bytes = original.clone();
        bytes[table + 8..table + 12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Directory::file(&bytes).is_err());
        let mut bytes = original;
        bytes[table + 2..table + 4].copy_from_slice(&0x580bu16.to_le_bytes());
        bytes[table + 4..table + 12].fill(255);
        assert_eq!(
            Directory::file(&bytes).unwrap().record(0).unwrap().payload,
            &[255; 8]
        );
    }
}
