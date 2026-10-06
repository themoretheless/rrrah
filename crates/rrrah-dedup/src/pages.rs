//! Complete ordered page/resource comparison; animated containers are excluded.
use crate::{
    animated::{AnimationBudget, AnimationError},
    decode::FileError,
    exact::ContentSnapshot,
    pixels::PixelError,
    raster::NormalizedRaster,
    sequence::SequenceError,
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;

#[derive(Debug, Clone, Copy)]
pub enum PageKind {
    Dcx,
    Tiff,
    Icon,
}

#[derive(Debug)]
pub struct Pages {
    pages: Vec<NormalizedRaster>,
}
impl Pages {
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
    /// Versioned candidate key for every ordered page, including selected-frame metadata.
    /// Equal keys require direct page confirmation; a key never proves identity.
    ///
    /// # Errors
    /// Invalid normalized data, work bounds or cancellation without a partial key.
    pub fn page_digest(&self, cancel: impl Fn() -> bool) -> Result<[u8; 32], AnimationError> {
        let mut hash = blake3::Hasher::new();
        hash.update(b"rrrah-ordered-pages-v1");
        hash.update(
            &u64::try_from(self.pages.len())
                .map_err(|_| SequenceError::Timing)?
                .to_le_bytes(),
        );
        for page in &self.pages {
            if cancel() {
                return Err(SequenceError::Pixels(PixelError::Cancelled).into());
            }
            hash.update(&page.selected_frame_digest(&cancel).map_err(FileError::from)?);
        }
        if cancel() {
            return Err(SequenceError::Pixels(PixelError::Cancelled).into());
        }
        Ok(*hash.finalize().as_bytes())
    }

    /// Exact ordered page equality; this does not prove byte-identical containers.
    ///
    /// # Errors
    /// Returns cancellation or invalid normalized data.
    pub fn same_pages(&self, other: &Self, cancel: impl Fn() -> bool) -> Result<bool, AnimationError> {
        if cancel() {
            return Err(SequenceError::Pixels(PixelError::Cancelled).into());
        }
        if self.pages.len() != other.pages.len() {
            return Ok(false);
        }
        for (a, b) in self.pages.iter().zip(&other.pages) {
            if !a.same_selected_frame(b, &cancel).map_err(FileError::from)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Decode every page under a count/memory budget, with full source validation
/// before and after processing. Explicit format magic prevents treating an
/// animation as untimed pages. All page ordinals/counts must remain consistent.
///
/// # Errors
/// Returns source/format/color/budget/cancellation errors without partial pages.
pub fn decode_pages(
    request: &DecodeRequest,
    kind: PageKind,
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<Pages, AnimationError> {
    if matches!(kind, PageKind::Icon) {
        return decode_icon_pages(request, limits, budget, cancel);
    }
    if matches!(kind, PageKind::Tiff) {
        return decode_tiff_pages(request, limits, budget, cancel);
    }
    if !matches!(kind, PageKind::Dcx) {
        return Err(AnimationError::UnsupportedPages);
    }
    let cancelled = || {
        cancel()
            || request
                .cancellation
                .as_ref()
                .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
    };
    let source = ContentSnapshot::read(&request.path, limits.max_file_bytes, cancelled)?;
    let magic: [u8; 4] = source
        .read_prefix(4, cancelled)?
        .try_into()
        .map_err(|_| SequenceError::Coverage)?;
    let valid = match kind {
        PageKind::Dcx => magic == [0xb1, 0x68, 0xde, 0x3a],
        PageKind::Tiff => matches!(magic, [b'I', b'I', 42 | 43, 0] | [b'M', b'M', 0, 42 | 43]),
        PageKind::Icon => matches!(magic, [0, 0, 1 | 2, 0]),
    };
    if !valid {
        return Err(SequenceError::Coverage.into());
    }
    let mut pages = Vec::new();
    let mut expected = None;
    let mut ordinal = 0;
    loop {
        if cancelled() {
            return Err(SequenceError::Pixels(PixelError::Cancelled).into());
        }
        if ordinal >= limits.max_frames {
            return Err(SequenceError::Coverage.into());
        }
        let mut selected = request.clone();
        selected.image_index = ordinal;
        selected.memory_budget = Some(budget.clone());
        let raster = rrrah_decode::decode_raster(&selected).map_err(FileError::from)?;
        let count = raster.image_count();
        if count == 0
            || count > limits.max_frames
            || raster.image_index() != ordinal
            || expected.is_some_and(|previous| previous != count)
        {
            return Err(SequenceError::Coverage.into());
        }
        expected = Some(count);
        if u64::from(raster.width()) * u64::from(raster.height()) > limits.max_pixels {
            return Err(SequenceError::Pixels(PixelError::Budget).into());
        }
        let linear =
            rrrah_decode::prepare_raster_for_display_with_budget_and_cancel(&raster, Some(budget), cancelled)
                .map_err(FileError::from)?;
        crate::local::reserve_slot(&mut pages, limits.max_frames)
            .map_err(|_| SequenceError::Pixels(PixelError::Budget))?;
        pages.push(
            NormalizedRaster::new(&linear, limits.max_pixels, budget, cancelled).map_err(FileError::from)?,
        );
        ordinal += 1;
        if ordinal == count {
            break;
        }
    }
    source.verify(cancelled)?;
    Ok(Pages { pages })
}

fn decode_icon_pages(
    request: &DecodeRequest,
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<Pages, AnimationError> {
    use crate::exact::SnapshotError;
    use std::io::Write;
    let cancelled = || {
        cancel()
            || request
                .cancellation
                .as_ref()
                .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
    };
    let source = ContentSnapshot::read(&request.path, limits.max_file_bytes, cancelled)?;
    let header = source.read_prefix(6, cancelled)?;
    if !matches!(header[..4], [0, 0, 1 | 2, 0]) {
        return Err(SequenceError::Coverage.into());
    }
    let count = usize::from(u16::from_le_bytes([header[4], header[5]]));
    if count == 0 || count > limits.max_frames {
        return Err(SequenceError::Coverage.into());
    }
    let end = count
        .checked_mul(16)
        .and_then(|n| n.checked_add(6))
        .ok_or(SequenceError::Coverage)?;
    let directory = source.read_prefix(end, cancelled)?;
    let mut pages = Vec::new();
    for (ordinal, entry) in directory[6..].as_chunks::<16>().0.iter().enumerate() {
        if cancelled() {
            return Err(SequenceError::Pixels(PixelError::Cancelled).into());
        }
        let length = u64::from(u32::from_le_bytes(
            entry[8..12].try_into().map_err(|_| SequenceError::Coverage)?,
        ));
        let offset = u64::from(u32::from_le_bytes(
            entry[12..16].try_into().map_err(|_| SequenceError::Coverage)?,
        ));
        if entry[3] != 0
            || length == 0
            || offset < end as u64
            || offset > source.byte_count()
            || length > source.byte_count() - offset
        {
            return Err(SequenceError::Coverage.into());
        }
        // Reuse the existing bitmap/PNG resource decoder and cursor hotspot rules.
        // Temporary encoded resources are streamed to disk, not held in RAM.
        let suffix = if header[2] == 2 { ".cur" } else { ".ico" };
        let mut temporary = tempfile::Builder::new()
            .suffix(suffix)
            .tempfile()
            .map_err(SnapshotError::from)?;
        let mut single = [0_u8; 22];
        single[..4].copy_from_slice(&header[..4]);
        single[4..6].copy_from_slice(&1_u16.to_le_bytes());
        single[6..].copy_from_slice(entry);
        single[18..22].copy_from_slice(&22_u32.to_le_bytes());
        temporary.write_all(&single).map_err(SnapshotError::from)?;
        source.copy_range(offset, length, temporary.as_file_mut(), cancelled)?;
        temporary.flush().map_err(SnapshotError::from)?;
        let mut selected = request.clone();
        selected.path = temporary.path().to_path_buf();
        selected.image_index = 0;
        selected.memory_budget = Some(budget.clone());
        let raster = rrrah_decode::decode_raster(&selected).map_err(FileError::from)?;
        if u64::from(raster.width()) * u64::from(raster.height()) > limits.max_pixels {
            return Err(SequenceError::Pixels(PixelError::Budget).into());
        }
        let raster = raster
            .with_image_selection(ordinal, count)
            .map_err(crate::raster::AdapterError::from)
            .map_err(FileError::from)?;
        let linear =
            rrrah_decode::prepare_raster_for_display_with_budget_and_cancel(&raster, Some(budget), cancelled)
                .map_err(FileError::from)?;
        crate::local::reserve_slot(&mut pages, limits.max_frames)
            .map_err(|_| SequenceError::Pixels(PixelError::Budget))?;
        pages.push(
            NormalizedRaster::new(&linear, limits.max_pixels, budget, cancelled).map_err(FileError::from)?,
        );
    }
    source.verify(cancelled)?;
    Ok(Pages { pages })
}

fn decode_tiff_pages(
    request: &DecodeRequest,
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<Pages, AnimationError> {
    decode_tiff_selection(request, limits, budget, cancel, None)
}

/// Decode one TIFF page with its own ICC profile and complete directory count.
/// Other pages are enumerated for coverage but are not decoded or retained.
///
/// # Errors
/// Returns invalid selection, directory/resource/color/source/cancellation errors.
pub fn decode_tiff_selected(
    request: &DecodeRequest,
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<NormalizedRaster, AnimationError> {
    let mut selected = decode_tiff_selection(request, limits, budget, cancel, Some(request.image_index))?;
    selected.pages.pop().ok_or_else(|| SequenceError::Coverage.into())
}

#[allow(clippy::too_many_lines)] // Bounded directory traversal and selected-page decoder orchestration.
fn decode_tiff_selection(
    request: &DecodeRequest,
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    selected_ordinal: Option<usize>,
) -> Result<Pages, AnimationError> {
    use crate::exact::SnapshotError;
    use std::{
        collections::HashSet,
        io::{Seek, SeekFrom, Write},
    };
    let cancelled = || {
        cancel()
            || request
                .cancellation
                .as_ref()
                .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
    };
    let source = ContentSnapshot::read(&request.path, limits.max_file_bytes, cancelled)?;
    let header = source.read_prefix(16, cancelled)?;
    let little = &header[..2] == b"II";
    if !little && &header[..2] != b"MM" {
        return Err(SequenceError::Coverage.into());
    }
    let number = |bytes: &[u8]| -> u64 {
        if little {
            bytes.iter().rev().fold(0, |n, b| (n << 8) | u64::from(*b))
        } else {
            bytes.iter().fold(0, |n, b| (n << 8) | u64::from(*b))
        }
    };
    let (pointer_at, pointer_bytes, count_bytes, entry_bytes) = match number(&header[2..4]) {
        42 => (4_usize, 4_usize, 2_u64, 12_u64),
        43 if number(&header[4..6]) == 8 && number(&header[6..8]) == 0 => (8, 8, 8, 20),
        _ => return Err(SequenceError::Coverage.into()),
    };
    let mut offset = number(&header[pointer_at..pointer_at + pointer_bytes]);
    let mut visited = HashSet::new();
    let mut directories = Vec::new();
    while offset != 0 {
        if cancelled() {
            return Err(SequenceError::Pixels(PixelError::Cancelled).into());
        }
        if directories.len() >= limits.max_frames || visited.contains(&offset) {
            return Err(SequenceError::Coverage.into());
        }
        visited
            .try_reserve(1)
            .map_err(|_| SequenceError::Pixels(PixelError::Budget))?;
        visited.insert(offset);
        let mut count = Vec::new();
        count
            .try_reserve_exact(usize::try_from(count_bytes).map_err(|_| SequenceError::Coverage)?)
            .map_err(|_| SequenceError::Pixels(PixelError::Budget))?;
        source.copy_range(offset, count_bytes, &mut count, cancelled)?;
        let count = number(&count);
        // Bound hostile directory work independently of the outer file size.
        if count > 4096 {
            return Err(SequenceError::Coverage.into());
        }
        let next_at = count
            .checked_mul(entry_bytes)
            .and_then(|n| n.checked_add(count_bytes))
            .and_then(|n| n.checked_add(offset))
            .ok_or(SequenceError::Coverage)?;
        let mut next = Vec::new();
        next.try_reserve_exact(pointer_bytes)
            .map_err(|_| SequenceError::Pixels(PixelError::Budget))?;
        source.copy_range(next_at, pointer_bytes as u64, &mut next, cancelled)?;
        crate::local::reserve_slot(&mut directories, limits.max_frames)
            .map_err(|_| SequenceError::Pixels(PixelError::Budget))?;
        directories.push((offset, next_at));
        offset = number(&next);
    }
    if directories.is_empty() {
        return Err(SequenceError::Coverage.into());
    }
    let count = directories.len();
    if selected_ordinal.is_some_and(|ordinal| ordinal >= count) {
        return Err(SequenceError::Coverage.into());
    }
    let mut pages = Vec::new();
    for (ordinal, (offset, next_at)) in directories.into_iter().enumerate() {
        if selected_ordinal.is_some_and(|selected| selected != ordinal) {
            continue;
        }
        let mut temporary = tempfile::Builder::new()
            .suffix(".tif")
            .tempfile()
            .map_err(SnapshotError::from)?;
        source.copy_range(0, source.byte_count(), temporary.as_file_mut(), cancelled)?;
        let pointer = if little {
            offset.to_le_bytes()
        } else {
            offset.to_be_bytes()
        };
        let pointer = if little {
            &pointer[..pointer_bytes]
        } else {
            &pointer[8 - pointer_bytes..]
        };
        temporary
            .seek(SeekFrom::Start(pointer_at as u64))
            .map_err(SnapshotError::from)?;
        temporary.write_all(pointer).map_err(SnapshotError::from)?;
        temporary
            .seek(SeekFrom::Start(next_at))
            .map_err(SnapshotError::from)?;
        temporary
            .write_all(&[0_u8; 8][..pointer_bytes])
            .map_err(SnapshotError::from)?;
        temporary.flush().map_err(SnapshotError::from)?;
        let mut selected = request.clone();
        selected.path = temporary.path().to_path_buf();
        selected.image_index = 0;
        selected.memory_budget = Some(budget.clone());
        let mut raster = rrrah_decode::decode_raster(&selected).map_err(FileError::from)?;
        if let Some((profile, reservation)) = tiff_profile(
            &source,
            offset,
            count_bytes,
            entry_bytes,
            pointer_bytes,
            &number,
            budget,
            &cancelled,
        )? {
            raster = rrrah_core::DecodedRaster::new(
                raster.width(),
                raster.height(),
                raster.pixels().clone(),
                rrrah_core::RasterColorSpace::Icc(profile),
            )
            .map_err(crate::raster::AdapterError::from)
            .map_err(FileError::from)?
            .with_color_profile_reservation(reservation)?;
        }
        if u64::from(raster.width()) * u64::from(raster.height()) > limits.max_pixels {
            return Err(SequenceError::Pixels(PixelError::Budget).into());
        }
        let raster = raster
            .with_image_selection(ordinal, count)
            .map_err(crate::raster::AdapterError::from)
            .map_err(FileError::from)?;
        let linear =
            rrrah_decode::prepare_raster_for_display_with_budget_and_cancel(&raster, Some(budget), cancelled)
                .map_err(FileError::from)?;
        crate::local::reserve_slot(&mut pages, limits.max_frames)
            .map_err(|_| SequenceError::Pixels(PixelError::Budget))?;
        pages.push(
            NormalizedRaster::new(&linear, limits.max_pixels, budget, cancelled).map_err(FileError::from)?,
        );
    }
    source.verify(cancelled)?;
    Ok(Pages { pages })
}

#[allow(clippy::too_many_arguments)] // Explicit TIFF layout and shared budgets.
fn tiff_profile(
    source: &ContentSnapshot,
    offset: u64,
    count_bytes: u64,
    entry_bytes: u64,
    pointer_bytes: usize,
    number: &impl Fn(&[u8]) -> u64,
    budget: &MemoryBudget,
    cancel: &impl Fn() -> bool,
) -> Result<Option<(Vec<u8>, rrrah_core::Reservation)>, AnimationError> {
    let mut count = Vec::new();
    count
        .try_reserve_exact(usize::try_from(count_bytes).map_err(|_| SequenceError::Coverage)?)
        .map_err(|_| SequenceError::Pixels(PixelError::Budget))?;
    source.copy_range(offset, count_bytes, &mut count, cancel)?;
    let length = number(&count)
        .checked_mul(entry_bytes)
        .ok_or(SequenceError::Coverage)?;
    let mut metadata_reservation = budget.try_reserve(length)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(usize::try_from(length).map_err(|_| SequenceError::Coverage)?)
        .map_err(|_| SequenceError::Pixels(PixelError::Budget))?;
    metadata_reservation
        .ensure_bytes(u64::try_from(entries.capacity()).map_err(|_| SequenceError::Coverage)?)?;
    source.copy_range(offset + count_bytes, length, &mut entries, cancel)?;
    let mut profile = None;
    for entry in entries.chunks_exact(usize::try_from(entry_bytes).map_err(|_| SequenceError::Coverage)?) {
        if number(&entry[..2]) != 34675 {
            continue;
        }
        if profile.is_some() || number(&entry[2..4]) != 7 {
            return Err(SequenceError::Coverage.into());
        }
        let field_at = if pointer_bytes == 4 { 8 } else { 12 };
        let size = number(&entry[4..field_at]);
        if size <= pointer_bytes as u64 {
            return Err(SequenceError::Coverage.into());
        }
        let at = number(&entry[field_at..]);
        if at > source.byte_count() || size > source.byte_count() - at {
            return Err(SequenceError::Coverage.into());
        }
        let mut reservation = budget.try_reserve(size)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(usize::try_from(size).map_err(|_| SequenceError::Coverage)?)
            .map_err(|_| SequenceError::Pixels(PixelError::Budget))?;
        reservation.ensure_bytes(u64::try_from(bytes.capacity()).map_err(|_| SequenceError::Coverage)?)?;
        source.copy_range(at, size, &mut bytes, cancel)?;
        profile = Some((bytes, reservation));
    }
    Ok(profile)
}

#[cfg(test)]
mod page_key_tests {
    use super::*;
    #[test]
    fn key_retains_page_order_and_count_independently_of_first_page() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiff");
        let request = DecodeRequest::new(root.join("classic-little-none-same.tif"));
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let limits = AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        };
        let a = decode_pages(&request, PageKind::Tiff, limits, &budget, || false).unwrap();
        let mut b = decode_pages(&request, PageKind::Tiff, limits, &budget, || false).unwrap();
        let key = a.page_digest(|| false).unwrap();
        b.pages.swap(1, 2);
        assert!(a.pages[0].same_selected_frame(&b.pages[0], || false).unwrap());
        assert!(!a.same_pages(&b, || false).unwrap());
        assert_ne!(key, b.page_digest(|| false).unwrap());
        b.pages.swap(1, 2);
        b.pages.pop();
        assert!(a.pages[0].same_selected_frame(&b.pages[0], || false).unwrap());
        assert!(!a.same_pages(&b, || false).unwrap());
        assert_ne!(key, b.page_digest(|| false).unwrap());
        drop((a, b));
        assert_eq!(budget.used(), 0);
    }
}
