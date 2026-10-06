//! Bounded content classification for complete presentation comparison.
use crate::{
    animated::{AnimationBudget, AnimationKind},
    container::{ContainerError, Presentation},
    exact::ContentSnapshot,
    pages::PageKind,
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;

/// Detect presentation scope from bytes, independent of filename extension.
/// PNG chunk traversal is bounded by `max_chunks`; payloads are skipped by offset.
/// Detection is not validation: the selected decoder validates full contents.
///
/// # Errors
/// Returns source/mutation/cancellation, routing or metadata-work-limit errors.
pub fn detect_presentation(
    request: &DecodeRequest,
    limits: AnimationBudget,
    max_chunks: usize,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<Presentation, ContainerError> {
    let cancelled = || {
        cancel()
            || request
                .cancellation
                .as_ref()
                .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
    };
    if cancelled() {
        return Err(ContainerError::Cancelled);
    }
    let source = ContentSnapshot::read(&request.path, limits.max_file_bytes, cancelled)?;
    let header = source.read_prefix(
        usize::try_from(source.byte_count().min(32)).map_err(|_| ContainerError::Detection)?,
        cancelled,
    )?;
    let mode = if header.starts_with(b"GIF87a") || header.starts_with(b"GIF89a") {
        Presentation::Animation(AnimationKind::Gif)
    } else if header.starts_with(b"\x89PNG\r\n\x1a\n") {
        png_kind(&source, max_chunks, cancelled)?
    } else if header.starts_with(b"RIFF") && header.get(8..12) == Some(b"WEBP") {
        if header.get(12..16) == Some(b"VP8X") && header.get(20).is_some_and(|flags| flags & 2 != 0) {
            Presentation::Animation(AnimationKind::Webp)
        } else {
            Presentation::SelectedFrame
        }
    } else if header.starts_with(&[0xb1, 0x68, 0xde, 0x3a]) {
        Presentation::Pages(PageKind::Dcx)
    } else if header.starts_with(&[0, 0, 1, 0]) || header.starts_with(&[0, 0, 2, 0]) {
        Presentation::Pages(PageKind::Icon)
    } else if matches!(
        header.get(..4),
        Some([b'I', b'I', 42 | 43, 0] | [b'M', b'M', 0, 42 | 43])
    ) {
        let mut classified = request.clone();
        classified.memory_budget = Some(budget.clone());
        if rrrah_decode::image_source_kind(&classified).map_err(crate::decode::FileError::from)?
            == rrrah_decode::ImageSourceKind::Sensor
        {
            Presentation::SelectedFrame
        } else {
            Presentation::Pages(PageKind::Tiff)
        }
    } else {
        Presentation::SelectedFrame
    };
    source.verify(cancelled)?;
    if cancelled() {
        return Err(ContainerError::Cancelled);
    }
    Ok(mode)
}

fn png_kind(
    source: &ContentSnapshot,
    max_chunks: usize,
    cancel: impl Fn() -> bool,
) -> Result<Presentation, ContainerError> {
    let mut offset = 8_u64;
    for _ in 0..max_chunks {
        let mut header = Vec::new();
        source.copy_range(offset, 8, &mut header, &cancel)?;
        let length = u64::from(u32::from_be_bytes(header[..4].try_into().expect("chunk length")));
        let end = offset
            .checked_add(length)
            .and_then(|n| n.checked_add(12))
            .ok_or(ContainerError::Detection)?;
        if end > source.byte_count() {
            return Err(ContainerError::Detection);
        }
        match &header[4..8] {
            b"acTL" if length == 8 => return Ok(Presentation::Animation(AnimationKind::Apng)),
            b"acTL" => return Err(ContainerError::Detection),
            b"IDAT" | b"IEND" => return Ok(Presentation::SelectedFrame),
            _ => offset = end,
        }
    }
    Err(ContainerError::Detection)
}
