//! Explicit-layout Casio BAY sensor unpacking, without camera/color inference.
//! Layout reference: https://www.inweb.ch/foto/rawformat.html

/// Full sample storage in headerless files; establish camera independently.
/// QV-5700 includes nine columns beyond its 2576-column active area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaySensorLayout {
    Qv2000Ux,
    Qv3000Ex,
    Qv5700,
}
impl BaySensorLayout {
    #[must_use]
    pub fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Qv2000Ux => (1632, 1211),
            Self::Qv3000Ex => (2080, 1547),
            Self::Qv5700 => (2585, 1924),
        }
    }
    /// Image-bearing samples in each row, excluding the QV-5700's 12-byte
    /// trailer. `dimensions()` describes legacy full-row unpack storage and
    /// includes nine unpacked trailer positions for that camera. This does
    /// not establish an optical-black crop or color interpretation.
    #[must_use]
    pub fn image_samples_per_row(self) -> usize {
        match self {
            Self::Qv5700 => 2576,
            _ => self.dimensions().0 as usize,
        }
    }

    /// Image-bearing geometry, excluding documented per-row trailer storage.
    #[must_use]
    pub fn image_dimensions(self) -> (u32, u32) {
        (self.image_samples_per_row() as u32, self.dimensions().1)
    }

    fn row_bytes(self) -> usize {
        match self {
            Self::Qv5700 => 3232,
            _ => self.dimensions().0 as usize,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum BaySensorError {
    #[error("BAY sensor byte length differs from the explicit camera layout")]
    Length,
    #[error("BAY sensor unpack cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
}
#[derive(Debug, thiserror::Error)]
pub enum BayReadError {
    #[error(transparent)]
    Source(#[from] crate::DecodeError),
    #[error(transparent)]
    Sensor(#[from] BaySensorError),
}
/// Stream explicit-layout BAY rows with scratch and output charged to one root.
/// The selected layout caps source admission before allocation. Oversized
/// sources return `DecodeError::InputTooLarge`; shorter sources fail the
/// sensor length check. Camera identity and color interpretation remain the
/// caller's responsibility.
pub fn read_bay_sensor_with_budget(
    request: &crate::DecodeRequest,
    layout: BaySensorLayout,
    budget: &rrrah_core::MemoryBudget,
) -> Result<rrrah_core::PixelBuffer<u16>, BayReadError> {
    read_bay_source(request, layout, budget, layout.dimensions().0 as usize)
}

/// Stream image-bearing sensor samples, excluding documented row trailers.
/// QV-5700 output is 2576x1924; the other explicit layouts keep their full
/// dimensions. This does not infer CFA, optical-black crop, WB or color.
pub fn read_bay_image_sensor_with_budget(
    request: &crate::DecodeRequest,
    layout: BaySensorLayout,
    budget: &rrrah_core::MemoryBudget,
) -> Result<rrrah_core::PixelBuffer<u16>, BayReadError> {
    read_bay_source(request, layout, budget, layout.image_samples_per_row())
}

fn read_bay_source(
    request: &crate::DecodeRequest,
    layout: BaySensorLayout,
    budget: &rrrah_core::MemoryBudget,
    width: usize,
) -> Result<rrrah_core::PixelBuffer<u16>, BayReadError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let (_, height) = layout.dimensions();
    let source_bytes = layout.row_bytes() as u64 * u64::from(height);
    let io_error = |source| crate::DecodeError::Io {
        path: request.path.clone(),
        source,
    };
    let mut file = std::fs::File::open(&request.path).map_err(io_error)?;
    let declared = file.metadata().map_err(io_error)?.len();
    if declared > source_bytes {
        return Err(crate::DecodeError::InputTooLarge {
            path: request.path.clone(),
            actual: declared,
            limit: source_bytes,
        }
        .into());
    }
    if declared != source_bytes {
        return Err(BaySensorError::Length.into());
    }
    read_bay_image_rows(&mut file, request, layout, budget, width)
}

#[cfg(test)]
fn read_bay_rows(
    file: &mut impl std::io::Read,
    request: &crate::DecodeRequest,
    layout: BaySensorLayout,
    budget: &rrrah_core::MemoryBudget,
) -> Result<rrrah_core::PixelBuffer<u16>, BayReadError> {
    read_bay_image_rows(file, request, layout, budget, layout.dimensions().0 as usize)
}

fn read_bay_image_rows(
    file: &mut impl std::io::Read,
    request: &crate::DecodeRequest,
    layout: BaySensorLayout,
    budget: &rrrah_core::MemoryBudget,
    width: usize,
) -> Result<rrrah_core::PixelBuffer<u16>, BayReadError> {
    request.check_cancelled()?;
    let (_, height) = layout.dimensions();
    let io_error = |source| crate::DecodeError::Io {
        path: request.path.clone(),
        source,
    };
    let mut row = budget
        .try_buffer(layout.row_bytes(), 0u8)
        .map_err(BaySensorError::from)?;
    let mut output = budget
        .try_buffer(width * height as usize, 0u16)
        .map_err(BaySensorError::from)?;
    for pixels in output.chunks_exact_mut(width) {
        request.check_cancelled()?;
        let mut remaining = &mut row[..];
        while !remaining.is_empty() {
            request.check_cancelled()?;
            match file.read(remaining) {
                Ok(0) => return Err(io_error(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof, "truncated BAY row",
                )).into()),
                Ok(count) => remaining = &mut remaining[count..],
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(io_error(error).into()),
            }
        }
        unpack_row(&row, pixels, layout);
    }
    request.check_cancelled()?;
    let mut extra = [0u8; 1];
    let trailing = loop {
        request.check_cancelled()?;
        match file.read(&mut extra) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => break result.map_err(io_error)?,
        }
    };
    if trailing != 0 {
        return Err(io_error(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "BAY source grew during streamed read",
        ))
        .into());
    }
    request.check_cancelled()?;
    Ok(output.freeze().into())
}
/// Sensor values only: no CFA, black/white levels, WB, crop or display color.
/// QV-5700 consumes 2585 ten-bit samples per 3232-byte row; six padding bits
/// remain after the final sample. There is no auto-detect or implicit crop.
pub fn unpack_bay_sensor(
    bytes: &[u8],
    layout: BaySensorLayout,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::PixelBuffer<u16>, BaySensorError> {
    if cancelled() {
        return Err(BaySensorError::Cancelled);
    }
    let (width, height) = layout.dimensions();
    let width = width as usize;
    let height = height as usize;
    if bytes.len() != layout.row_bytes() * height {
        return Err(BaySensorError::Length);
    }
    let mut output = budget.try_buffer(width * height, 0u16)?;
    for (row, pixels) in bytes
        .chunks_exact(layout.row_bytes())
        .zip(output.chunks_exact_mut(width))
    {
        if cancelled() {
            return Err(BaySensorError::Cancelled);
        }
        unpack_row(row, pixels, layout);
    }
    if cancelled() {
        return Err(BaySensorError::Cancelled);
    }
    Ok(output.freeze().into())
}

fn unpack_row(row: &[u8], pixels: &mut [u16], layout: BaySensorLayout) {
    let width = pixels.len();
    if layout == BaySensorLayout::Qv5700 {
        for (group, samples) in row[..width * 10 / 8]
            .chunks_exact(5)
            .zip(pixels.chunks_exact_mut(4))
        {
            let packed = u64::from_be_bytes([0, 0, 0, group[0], group[1], group[2], group[3], group[4]]);
            for (index, sample) in samples.iter_mut().enumerate() {
                *sample = ((packed >> (30 - index * 10)) & 1023) as u16;
            }
        }
        // 2584 samples occupy 3230 bytes; the last sample is the high ten
        // bits of the remaining two bytes, independent of six padding bits.
        if width == layout.dimensions().0 as usize {
            pixels[width - 1] = u16::from_be_bytes([row[3230], row[3231]]) >> 6;
        }
    } else {
        for (sample, &value) in pixels.iter_mut().zip(row) {
            *sample = u16::from(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qv5700_trailer_changes_do_not_change_image_samples() {
        let layout = BaySensorLayout::Qv5700;
        let (width, height) = layout.dimensions();
        let mut bytes = vec![0x59; layout.row_bytes() * height as usize];
        let budget = rrrah_core::MemoryBudget::new(u64::from(width) * u64::from(height) * 4);
        let first = unpack_bay_sensor(&bytes, layout, &budget, || false).unwrap();
        let image_bytes = layout.image_samples_per_row() * 10 / 8;
        assert_eq!(layout.row_bytes() - image_bytes, 12);
        for (row_index, row) in bytes.chunks_exact_mut(layout.row_bytes()).enumerate() {
            for (column, byte) in row[image_bytes..].iter_mut().enumerate() {
                *byte = (row_index + column) as u8;
            }
        }
        let second = unpack_bay_sensor(&bytes, layout, &budget, || false).unwrap();
        let mut trailer_changed = false;
        for (a, b) in first
            .chunks_exact(width as usize)
            .zip(second.chunks_exact(width as usize))
        {
            assert_eq!(
                &a[..layout.image_samples_per_row()],
                &b[..layout.image_samples_per_row()]
            );
            trailer_changed |= a[layout.image_samples_per_row()..] != b[layout.image_samples_per_row()..];
        }
        assert!(
            trailer_changed,
            "storage trailer samples must not be presented as image pixels"
        );
        drop((first, second));
        assert_eq!(budget.used(), 0);
    }

    #[test]
    #[ignore = "requires pinned authored BAY and independent LibRaw dumps at RRRAH_BAY_AUTHORED_CORPUS; not camera/color qualification"]
    fn authored_full_sensors_match_independent_libraw_oracles() {
        let root =
            std::path::PathBuf::from(std::env::var_os("RRRAH_BAY_AUTHORED_CORPUS").expect("authored corpus"));
        let manifest: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/bay-authored-oracles.json"
        )))
        .unwrap();
        let layouts = [
            BaySensorLayout::Qv2000Ux,
            BaySensorLayout::Qv3000Ex,
            BaySensorLayout::Qv5700,
        ];
        let fixtures = manifest["fixtures"].as_array().unwrap();
        assert_eq!(fixtures.len(), layouts.len());
        for (fixture, layout) in fixtures.iter().zip(layouts) {
            for (file, digest) in [("source", "source_sha256"), ("oracle", "oracle_sha256")] {
                let path = root.join(fixture[file].as_str().unwrap());
                let result = std::process::Command::new("shasum")
                    .args(["-a", "256"])
                    .arg(path)
                    .output()
                    .unwrap();
                assert!(result.status.success());
                assert_eq!(
                    String::from_utf8(result.stdout)
                        .unwrap()
                        .split_whitespace()
                        .next()
                        .unwrap(),
                    fixture[digest].as_str().unwrap()
                );
            }
            let (width, height) = layout.dimensions();
            let bytes = u64::from(width) * u64::from(height) * 2;
            let budget = rrrah_core::MemoryBudget::new(bytes + layout.row_bytes() as u64);
            let source = root.join(fixture["source"].as_str().unwrap());
            let output =
                read_bay_sensor_with_budget(&crate::DecodeRequest::new(source), layout, &budget).unwrap();
            let reference = std::fs::read(root.join(fixture["oracle"].as_str().unwrap())).unwrap();
            assert_eq!(reference.len() as u64, bytes);
            for (i, (&actual, expected)) in output.iter().zip(reference.chunks_exact(2)).enumerate() {
                assert_eq!(
                    actual,
                    u16::from_le_bytes([expected[0], expected[1]]),
                    "{layout:?} sample {i}"
                );
            }
            assert_eq!(budget.peak(), budget.limit());
            drop(output);
            assert_eq!(budget.used(), 0);
            eprintln!(
                "BAY independent authored oracle: {layout:?}; samples={}; every sample exact; final=0; original_camera_color_unqualified",
                width as u64 * height as u64
            );
        }
    }
    #[test]
    #[ignore = "requires pinned authored BAY and independent dumps at RRRAH_BAY_AUTHORED_CORPUS"]
    fn image_rows_match_independent_oracles_without_trailer_or_full_copy() {
        let root =
            std::path::PathBuf::from(std::env::var_os("RRRAH_BAY_AUTHORED_CORPUS").expect("authored corpus"));
        let manifest: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/bay-authored-oracles.json"
        )))
        .unwrap();
        for (fixture, layout) in manifest["fixtures"].as_array().unwrap().iter().zip([
            BaySensorLayout::Qv2000Ux,
            BaySensorLayout::Qv3000Ex,
            BaySensorLayout::Qv5700,
        ]) {
            let (width, height) = layout.image_dimensions();
            let bytes = u64::from(width) * u64::from(height) * 2;
            let total = bytes + layout.row_bytes() as u64;
            let request = crate::DecodeRequest::new(root.join(fixture["source"].as_str().unwrap()));
            let short = rrrah_core::MemoryBudget::new(total - 1);
            assert!(read_bay_image_sensor_with_budget(&request, layout, &short).is_err());
            assert_eq!(short.used(), 0);
            let budget = rrrah_core::MemoryBudget::new(total);
            let output = read_bay_image_sensor_with_budget(&request, layout, &budget).unwrap();
            assert_eq!(output.len(), width as usize * height as usize);
            let reference = std::fs::read(root.join(fixture["oracle"].as_str().unwrap())).unwrap();
            for (actual, row) in output
                .chunks_exact(width as usize)
                .zip(reference.chunks_exact(layout.dimensions().0 as usize * 2))
            {
                for (&value, expected) in actual.iter().zip(row.chunks_exact(2)) {
                    assert_eq!(value, u16::from_le_bytes([expected[0], expected[1]]));
                }
            }
            assert_eq!(budget.peak(), total);
            assert_eq!(budget.used(), bytes);
            drop(output);
            assert_eq!(budget.used(), 0);
            eprintln!(
                "BAY image-only oracle: {layout:?}; {width}x{height}; every sample exact; output+one-row peak={total}; final=0"
            );
        }
    }

    #[test]
    fn interrupted_stream_retries_eof_and_checks_cancellation_inside_rows() {
        use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
        struct Reader {
            inner: std::io::Cursor<Vec<u8>>,
            generation: Arc<AtomicU64>,
            eof_interrupt: bool,
            cancel_after: Option<usize>,
            calls: usize,
        }
        impl std::io::Read for Reader {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                self.calls += 1;
                if self.cancel_after.is_some() {
                    if self.cancel_after == Some(self.calls) { self.generation.store(2, Ordering::Release); }
                    if self.calls > 10 { panic!("cancelled read was retried indefinitely"); }
                    return Err(std::io::ErrorKind::Interrupted.into());
                }
                if self.inner.position() == self.inner.get_ref().len() as u64 && self.eof_interrupt {
                    self.eof_interrupt = false;
                    return Err(std::io::ErrorKind::Interrupted.into());
                }
                let length = out.len().min(17);
                std::io::Read::read(&mut self.inner, &mut out[..length])
            }
        }
        for layout in [BaySensorLayout::Qv2000Ux, BaySensorLayout::Qv3000Ex, BaySensorLayout::Qv5700] {
            let (width, height) = layout.image_dimensions();
            let peak = u64::from(width)*u64::from(height)*2 + layout.row_bytes() as u64;
            let generation = Arc::new(AtomicU64::new(1));
            let mut request = crate::DecodeRequest::new("interrupted.bay");
            request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 1));
            let budget = rrrah_core::MemoryBudget::new(peak);
            let mut reader = Reader { inner: std::io::Cursor::new(vec![173; layout.row_bytes()*height as usize]), generation: generation.clone(), eof_interrupt: true, cancel_after: Some(3), calls: 0 };
            assert!(matches!(read_bay_image_rows(&mut reader, &request, layout, &budget, width as usize), Err(BayReadError::Source(crate::DecodeError::Cancelled))));
            assert_eq!(reader.calls, 3);
            assert_eq!(budget.used(), 0);
            generation.store(1, Ordering::Release);
            reader.cancel_after = None;
            let result = read_bay_image_rows(&mut reader, &request, layout, &budget, width as usize).unwrap();
            assert!(!reader.eof_interrupt);
            let mut expected_row = vec![0; width as usize];
            unpack_row(&vec![173; layout.row_bytes()], &mut expected_row, layout);
            for row in result.chunks_exact(width as usize) { assert_eq!(row, expected_row); }
            assert_eq!(budget.peak(), peak);
            drop(result);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn streamed_truncation_growth_and_cancellation_release_partial_sensor() {
        use std::sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        };
        struct CancelRead {
            bytes: std::io::Cursor<Vec<u8>>,
            generation: Arc<AtomicU64>,
            calls: usize,
            cancel_at: usize,
        }
        impl std::io::Read for CancelRead {
            fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
                self.calls += 1;
                let count = std::io::Read::read(&mut self.bytes, output)?;
                if self.calls == self.cancel_at {
                    self.generation.store(2, Ordering::Release);
                }
                Ok(count)
            }
        }
        for (layout, image_only) in [
            (BaySensorLayout::Qv2000Ux, false),
            (BaySensorLayout::Qv3000Ex, true),
            (BaySensorLayout::Qv5700, false),
            (BaySensorLayout::Qv5700, true),
        ] {
            let (width, height) = if image_only {
                layout.image_dimensions()
            } else {
                layout.dimensions()
            };
            let read = |input: &mut CancelRead,
                        request: &crate::DecodeRequest,
                        budget: &rrrah_core::MemoryBudget| {
                if image_only {
                    read_bay_image_rows(input, request, layout, budget, width as usize)
                } else {
                    read_bay_rows(input, request, layout, budget)
                }
            };
            let bytes = layout.row_bytes() * height as usize;
            let capacity = u64::from(width) * u64::from(height) * 2 + layout.row_bytes() as u64;
            // The final read checks EOF; cancellation there must still refuse publication.
            for cancel_at in [1, height as usize / 2, height as usize, height as usize + 1] {
                let generation = Arc::new(AtomicU64::new(1));
                let mut request = crate::DecodeRequest::new("authored-stream.bay");
                request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 1));
                let budget = rrrah_core::MemoryBudget::new(capacity);
                let mut input = CancelRead {
                    bytes: std::io::Cursor::new(vec![173; bytes]),
                    generation,
                    calls: 0,
                    cancel_at,
                };
                assert!(matches!(
                    read(&mut input, &request, &budget),
                    Err(BayReadError::Source(crate::DecodeError::Cancelled))
                ));
                assert_eq!(budget.peak(), capacity);
                assert_eq!(budget.used(), 0);
                request.cancellation = None;
                let mut retry = std::io::Cursor::new(vec![173; bytes]);
                let output =
                    read_bay_image_rows(&mut retry, &request, layout, &budget, width as usize).unwrap();
                assert_eq!(output.len(), width as usize * height as usize);
                assert_eq!(budget.used(), u64::from(width) * u64::from(height) * 2);
                drop(output);
                assert_eq!(budget.used(), 0);
            }
            for length in [bytes - 1, bytes + 1] {
                let budget = rrrah_core::MemoryBudget::new(capacity);
                let mut input = std::io::Cursor::new(vec![173; length]);
                assert!(matches!(
                    read_bay_image_rows(
                        &mut input,
                        &crate::DecodeRequest::new("changed.bay"),
                        layout,
                        &budget,
                        width as usize
                    ),
                    Err(BayReadError::Source(crate::DecodeError::Io { .. }))
                ));
                assert_eq!(budget.peak(), capacity);
                assert_eq!(budget.used(), 0);
            }
            eprintln!(
                "BAY streamed recovery: {layout:?}; image_only={image_only}; four admitted cancellations and same-budget retries, truncation/growth refused; final=0"
            );
        }
    }
    #[test]
    fn streamed_files_match_every_authored_sample_with_one_row_scratch() {
        let path = std::env::temp_dir().join(format!("rrrah-bay-stream-{}.bay", std::process::id()));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        for layout in [
            BaySensorLayout::Qv2000Ux,
            BaySensorLayout::Qv3000Ex,
            BaySensorLayout::Qv5700,
        ] {
            let (width, height) = layout.dimensions();
            let width = width as usize;
            let mut source = vec![0u8; layout.row_bytes() * height as usize];
            for (y, row) in source.chunks_exact_mut(layout.row_bytes()).enumerate() {
                if layout == BaySensorLayout::Qv5700 {
                    row[3231] = 0x3f; // Nonzero trailing padding must not enter samples.
                    for x in 0..width {
                        let value = (y * 17 + x) % 1024;
                        for bit in 0..10 {
                            if value & (1 << (9 - bit)) != 0 {
                                let position = x * 10 + bit;
                                row[position / 8] |= 1 << (7 - position % 8);
                            }
                        }
                    }
                } else {
                    for (x, byte) in row.iter_mut().enumerate() {
                        *byte = ((y * 17 + x) % 256) as u8;
                    }
                }
            }
            std::fs::write(&path, &source).unwrap();
            let output_bytes = width as u64 * u64::from(height) * 2;
            let budget = rrrah_core::MemoryBudget::new(output_bytes + layout.row_bytes() as u64);
            let pixels =
                read_bay_sensor_with_budget(&crate::DecodeRequest::new(&path), layout, &budget).unwrap();
            let modulus = if layout == BaySensorLayout::Qv5700 {
                1024
            } else {
                256
            };
            for (i, &sample) in pixels.iter().enumerate() {
                assert_eq!(usize::from(sample), ((i / width) * 17 + i % width) % modulus);
            }
            assert_eq!(budget.peak(), budget.limit());
            assert_eq!(budget.used(), output_bytes);
            drop(pixels);
            assert_eq!(budget.used(), 0);
            let image_width = layout.image_samples_per_row();
            let image_bytes = image_width as u64 * u64::from(height) * 2;
            let image_budget = rrrah_core::MemoryBudget::new(image_bytes + layout.row_bytes() as u64);
            let image =
                read_bay_image_sensor_with_budget(&crate::DecodeRequest::new(&path), layout, &image_budget)
                    .unwrap();
            assert_eq!(image.len(), image_width * height as usize);
            for (i, &sample) in image.iter().enumerate() {
                assert_eq!(
                    usize::from(sample),
                    ((i / image_width) * 17 + i % image_width) % modulus
                );
            }
            assert_eq!(image_budget.peak(), image_budget.limit());
            drop(image);
            assert_eq!(image_budget.used(), 0);
        }
    }
    #[test]
    fn oversized_file_is_refused_before_source_admission_for_every_layout() {
        let path = std::env::temp_dir().join(format!("rrrah-bay-oversized-{}.bay", std::process::id()));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        for layout in [
            BaySensorLayout::Qv2000Ux,
            BaySensorLayout::Qv3000Ex,
            BaySensorLayout::Qv5700,
        ] {
            let (_, height) = layout.dimensions();
            let expected = layout.row_bytes() as u64 * u64::from(height);
            std::fs::File::create(&path)
                .unwrap()
                .set_len(expected + 1)
                .unwrap();
            let root = rrrah_core::MemoryBudget::new(1);
            let request = crate::DecodeRequest::new(path.clone());
            assert!(matches!(read_bay_sensor_with_budget(&request, layout, &root),
                Err(BayReadError::Source(crate::DecodeError::InputTooLarge { actual, limit, .. }))
                if actual == expected + 1 && limit == expected));
            assert_eq!(root.peak(), 0);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn file_source_and_sensor_share_budget_and_release_after_refusal() {
        let layout = BaySensorLayout::Qv2000Ux;
        let (width, height) = layout.dimensions();
        let len = (width * height) as usize;
        let path = std::env::temp_dir().join(format!(
            "rrrah-bay-input-{}-{}.bay",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        std::fs::write(&path, vec![173u8; len]).unwrap();
        let mut request = crate::DecodeRequest::new(path.clone());
        let exact_bytes = len as u64 * 2 + layout.row_bytes() as u64;
        let exact = rrrah_core::MemoryBudget::new(exact_bytes);
        let output = read_bay_sensor_with_budget(&request, layout, &exact).unwrap();
        assert!(output.iter().all(|sample| *sample == 173));
        assert_eq!(exact.peak(), exact_bytes);
        assert_eq!(exact.used(), len as u64 * 2);
        let clone = output.clone();
        drop(output);
        assert_eq!(exact.used(), len as u64 * 2);
        drop(clone);
        assert_eq!(exact.used(), 0);
        let tight = rrrah_core::MemoryBudget::new(exact_bytes - 1);
        assert!(matches!(
            read_bay_sensor_with_budget(&request, layout, &tight),
            Err(BayReadError::Sensor(BaySensorError::Memory(_)))
        ));
        assert_eq!(tight.peak(), layout.row_bytes() as u64);
        assert_eq!(tight.used(), 0);
        request.image_index = 1;
        let fresh = rrrah_core::MemoryBudget::new(len as u64 * 3);
        assert!(matches!(
            read_bay_sensor_with_budget(&request, layout, &fresh),
            Err(BayReadError::Source(crate::DecodeError::UnsupportedImageIndex {
                index: 1
            }))
        ));
        assert_eq!(fresh.peak(), 0);
        request.image_index = 0;
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
            0,
        ));
        assert!(matches!(
            read_bay_sensor_with_budget(&request, layout, &fresh),
            Err(BayReadError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!(fresh.peak(), 0);
        request.cancellation = None;
        std::fs::write(path, [0u8]).unwrap();
        assert!(matches!(
            read_bay_sensor_with_budget(&request, layout, &fresh),
            Err(BayReadError::Sensor(BaySensorError::Length))
        ));
        assert_eq!(fresh.used(), 0);
    }
    #[test]
    fn all_ten_bit_values_storage_columns_padding_and_managed_ownership() {
        let layout = BaySensorLayout::Qv5700;
        let (width, height) = layout.dimensions();
        let mut source = vec![0xa5; layout.row_bytes() * height as usize];
        // Independent MSB-first fixture bit writer; all 1024 values and row
        // boundaries and all nine extra storage columns are exercised.
        for row in 0..height as usize {
            source[row * 3232..(row + 1) * 3232].fill(0);
            source[(row + 1) * 3232 - 1] = 0x3f;
            for column in 0..width as usize {
                let value = (row + column) % 1024;
                for bit in 0..10 {
                    if value & (1 << (9 - bit)) != 0 {
                        let position = column * 10 + bit;
                        source[row * 3232 + position / 8] |= 1 << (7 - position % 8);
                    }
                }
            }
        }
        let budget = rrrah_core::MemoryBudget::new(u64::from(width) * u64::from(height) * 2);
        let pixels = unpack_bay_sensor(&source, layout, &budget, || false).unwrap();
        for (index, &value) in pixels.iter().enumerate() {
            assert_eq!(
                usize::from(value),
                (index / width as usize + index % width as usize) % 1024
            );
        }
        assert_eq!(budget.used(), u64::from(width) * u64::from(height) * 2);
        let shared = pixels.clone();
        drop(pixels);
        assert!(budget.used() > 0);
        drop(shared);
        assert_eq!(budget.used(), 0);
        for size in [source.len() - 1, source.len() + 1] {
            let mut invalid = source.clone();
            invalid.resize(size, 0);
            assert!(matches!(
                unpack_bay_sensor(&invalid, layout, &budget, || false),
                Err(BaySensorError::Length)
            ));
            assert_eq!(budget.used(), 0);
        }
        let tiny = rrrah_core::MemoryBudget::new(1);
        assert!(matches!(
            unpack_bay_sensor(&source, layout, &tiny, || false),
            Err(BaySensorError::Memory(_))
        ));
        assert_eq!(tiny.peak(), 0);
        let mut calls = 0;
        assert!(matches!(
            unpack_bay_sensor(&source, layout, &budget, || {
                calls += 1;
                calls == 3
            }),
            Err(BaySensorError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn eight_bit_layouts_preserve_all_values_and_exact_storage_dimensions() {
        for layout in [BaySensorLayout::Qv2000Ux, BaySensorLayout::Qv3000Ex] {
            let (width, height) = layout.dimensions();
            let source: Vec<_> = (0..width as usize * height as usize)
                .map(|index| (index % 256) as u8)
                .collect();
            let budget = rrrah_core::MemoryBudget::new(source.len() as u64 * 2);
            let samples = unpack_bay_sensor(&source, layout, &budget, || false).unwrap();
            assert_eq!(samples.len(), source.len());
            for (sample, value) in samples.iter().zip(&source) {
                assert_eq!(*sample, u16::from(*value));
            }
            drop(samples);
            assert_eq!(budget.used(), 0);
            assert!(matches!(
                unpack_bay_sensor(&source, layout, &budget, || true),
                Err(BaySensorError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
    }
}
