//! Encoded-file adapter using rrrah's existing format routing and decoders.

use crate::{
    pixels::PixelError,
    raster::{AdapterError, NormalizedRaster},
};
use rrrah_core::MemoryBudget;
use rrrah_decode::{DecodeRequest, DecodedImage, RasterDecodeError};

/// Source observations retained by decode, index and presentation operations.
#[derive(Debug)]
pub(crate) struct DecodeSourceSnapshot {
    source: crate::exact::ContentSnapshot,
    dependencies: Vec<crate::exact::ContentSnapshot>,
}
impl DecodeSourceSnapshot {
    pub(crate) fn read(path: &std::path::Path, max_bytes: u64, cancel: impl Fn() -> bool)
        -> Result<Self, crate::exact::SnapshotError>
    {
        let source = crate::exact::ContentSnapshot::read(path, max_bytes, &cancel)?;
        let length = usize::try_from(source.byte_count().min(64))
            .map_err(|_| crate::exact::SnapshotError::Policy)?;
        let header = source.read_prefix(length, &cancel)?;
        let paths = rrrah_decode::raster_external_dependencies(&DecodeRequest::new(path), &header);
        let mut dependencies = Vec::new();
        dependencies.try_reserve_exact(paths.len()).map_err(|_| crate::exact::SnapshotError::Policy)?;
        for path in paths {
            dependencies.push(crate::exact::ContentSnapshot::read(&path, max_bytes, &cancel)?);
        }
        Ok(Self { source, dependencies })
    }
    pub(crate) fn verify(&self, cancel: impl Fn() -> bool) -> Result<(), crate::exact::SnapshotError> {
        self.source.verify(&cancel)?;
        for dependency in &self.dependencies { dependency.verify(&cancel)?; }
        Ok(())
    }
}
impl std::ops::Deref for DecodeSourceSnapshot {
    type Target = crate::exact::ContentSnapshot;
    fn deref(&self) -> &Self::Target { &self.source }
}

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error(transparent)]
    Decode(#[from] RasterDecodeError),
    #[error(transparent)]
    Normalize(#[from] AdapterError),
    #[error(transparent)]
    RawDecode(#[from] rrrah_decode::DecodeError),
    #[error(transparent)]
    Color(#[from] rrrah_decode::RasterColorError),
    #[error(transparent)]
    Develop(#[from] rrrah_core::develop::DevelopError),
    #[error(transparent)]
    Prepared(Box<CachedError>),
}

/// Decode the requested frame and normalize using the existing raster pipeline.
/// The same memory budget admits both supported decoder and conversion buffers.
/// A successful result covers the selected frame, not an entire animation.
/// Sensor previews are never substituted for full-resolution pixel evidence.
///
/// # Errors
/// Returns decoder, color, budget and cancellation errors explicitly. Sensor
/// development failures are reported rather than producing a false match.
/// Callback cancellation is checked before/after decoding; use the request's
/// generation token for supported decoder-internal cancellation points.
/// Uses a 1 GiB encoded-source limit and at most 4096 TIFF directories.
/// Use [`decode_selected_frame_bounded`] to supply different source/page limits.
pub fn decode_selected_frame(
    request: &DecodeRequest,
    max_pixels: u64,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<NormalizedRaster, FileError> {
    decode_selected_frame_bounded(
        request,
        crate::animated::AnimationBudget {
            max_frames: 4096,
            max_pixels,
            max_file_bytes: 1024 * 1024 * 1024,
        },
        budget,
        cancel,
    )
    .map_err(|error| match error {
        CachedError::Decode(error) => error,
        other => FileError::Prepared(Box::new(other)),
    })
}

/// Decode a selected frame with explicit source, pixel and directory limits.
/// TIFF applies the selected page's ICC, preserving its original index/count.
/// RAW TIFF containers retain authoritative sensor routing. Source observations
/// bracket decoding; they are not an atomic filesystem snapshot.
///
/// # Errors
/// Returns source, routing, directory, color, memory or cancellation errors.
pub fn decode_selected_frame_bounded(
    request: &DecodeRequest,
    limits: crate::animated::AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<NormalizedRaster, CachedError> {
    let cancelled = || {
        cancel()
            || request
                .cancellation
                .as_ref()
                .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
    };
    let source = DecodeSourceSnapshot::read(&request.path, limits.max_file_bytes, cancelled)?;
    decode_snapshot_frame(
        request,
        &source,
        FingerprintPolicy {
            recipe: [0; 32],
            max_file_bytes: limits.max_file_bytes,
            max_pixels: limits.max_pixels,
            max_frames: limits.max_frames,
            max_cache_entries: 0,
        },
        budget,
        cancelled,
    )
}

fn decode_native_frame(
    request: &DecodeRequest,
    max_pixels: u64,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<NormalizedRaster, FileError> {
    let cancelled = || {
        cancel()
            || request
                .cancellation
                .as_ref()
                .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
    };
    if cancelled() {
        return Err(AdapterError::Pixels(PixelError::Cancelled).into());
    }
    let mut bounded = request.clone();
    bounded.memory_budget = Some(budget.clone());
    match rrrah_decode::decode_image(&bounded)? {
        DecodedImage::Raster(raster) => {
            if u64::from(raster.width()) * u64::from(raster.height()) > max_pixels {
                return Err(AdapterError::Pixels(PixelError::Budget).into());
            }
            let color = rrrah_decode::prepare_raster_for_display_with_budget_and_cancel(
                &raster,
                Some(budget),
                cancelled,
            )?;
            Ok(NormalizedRaster::new(&color, max_pixels, budget, cancelled)?)
        }
        DecodedImage::Sensor(sensor) => {
            if u64::from(sensor.mosaic.metadata.width) * u64::from(sensor.mosaic.metadata.height) > max_pixels
            {
                return Err(AdapterError::Pixels(PixelError::Budget).into());
            }
            let lists = rrrah_decode::raw_development_opcodes(&bounded)?;
            let raster = rrrah_core::develop::develop_raw(
                &sensor.mosaic,
                &rrrah_core::develop::DevelopOptions::default(),
                &lists,
                Some(budget),
                &cancelled,
            )?;
            Ok(NormalizedRaster::new(&raster, max_pixels, budget, cancelled)?)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CachedError {
    #[error(transparent)]
    Container(#[from] crate::animated::AnimationError),
    #[error(transparent)]
    Source(#[from] crate::exact::SnapshotError),
    #[error(transparent)]
    Decode(#[from] FileError),
    #[error(transparent)]
    Cache(#[from] crate::cache::CacheError),
    #[error(transparent)]
    Pixels(#[from] PixelError),
    #[error(transparent)]
    Normalize(#[from] AdapterError),
}

#[derive(Debug, Clone, Copy)]
pub struct FingerprintPolicy {
    /// Include decoder/color/development versions and options in this digest.
    pub recipe: [u8; 32],
    pub max_file_bytes: u64,
    pub max_pixels: u64,
    /// Maximum TIFF directory count, including non-selected pages.
    pub max_frames: usize,
    pub max_cache_entries: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedFingerprint {
    pub fingerprint: crate::pixels::Fingerprint,
    pub cache_hit: bool,
}

/// Fingerprint one selected frame with content-addressed cache reuse. On misses,
/// source identity and full content are verified again after decoding; failed
/// processing never inserts an entry. Neither result proves pixel/file equality.
/// Cache hits still hash and validate the entire current source. The recipe
/// must change when decoder/color/development behavior changes.
///
/// # Errors
/// Returns I/O, source mutation, decoder, resource and cancellation errors.
pub fn fingerprint_file(
    request: &DecodeRequest,
    policy: FingerprintPolicy,
    budget: &MemoryBudget,
    cache: &mut crate::cache::FingerprintCache,
    cancel: impl Fn() -> bool,
) -> Result<CachedFingerprint, CachedError> {
    let cancelled = || {
        cancel()
            || request
                .cancellation
                .as_ref()
                .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
    };
    let source = DecodeSourceSnapshot::read(&request.path, policy.max_file_bytes, cancelled)?;
    let header_length = usize::try_from(source.byte_count().min(64))
        .map_err(|_| crate::cache::CacheError::Invalid)?;
    let header = source.read_prefix(header_length, cancelled)?;
    let dependencies = &source.dependencies;
    let native_identity = if header.get(4..8) == Some(b"ftyp") {
        Some(rrrah_decode::hevc_decoder_resource_identity().map_err(FileError::from)?)
    } else {
        None
    };
    let verify_native = || -> Result<(), CachedError> {
        if let Some(expected) = native_identity {
            let current = rrrah_decode::hevc_decoder_resource_identity().map_err(FileError::from)?;
            if current != expected {
                return Err(FileError::from(rrrah_decode::RasterDecodeError::InvalidHeif(
                    "decoder inventory changed during fingerprint processing".into(),
                )).into());
            }
        }
        Ok(())
    };
    let mut recipe = blake3::Hasher::new();
    recipe.update(b"rrrah-file-linear-fingerprint-v5");
    recipe.update(env!("RRRAH_BUILT_RECIPE_ID").as_bytes());
    recipe.update(&policy.recipe);
    recipe.update(b"external-file-dependencies-v1");
    recipe.update(&(dependencies.len() as u64).to_le_bytes());
    for dependency in dependencies {
        recipe.update(&dependency.digest());
    }
    // Native dispatch and selected-frame admission can depend on the extension.
    // Keep content-addressed reuse across paths with the same dispatch hint.
    let extension = request.path.extension().map_or(&[][..], std::ffi::OsStr::as_encoded_bytes);
    recipe.update(b"native-extension-hint-v1");
    recipe.update(&(extension.len() as u64).to_le_bytes());
    for byte in extension {
        recipe.update(&[byte.to_ascii_lowercase()]);
    }
    if let Some(identity) = native_identity {
        recipe.update(b"hevc-runtime-inventory-v1");
        recipe.update(&identity);
    }
    if request.path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
        let fonts = rrrah_decode::svg_font_resource_identity().map_err(FileError::from)?;
        recipe.update(b"svg-font-resources-v1");
        recipe.update(&fonts);
    }
    recipe.update(&[u8::from(request.assume_untagged_srgb)]);
    recipe.update(&[u8::from(request.assume_untagged_linear_srgb)]);
    recipe.update(&policy.max_pixels.to_le_bytes());
    recipe.update(&policy.max_file_bytes.to_le_bytes());
    recipe.update(
        &u64::try_from(policy.max_frames)
            .map_err(|_| crate::cache::CacheError::Invalid)?
            .to_le_bytes(),
    );
    let key = crate::cache::CacheKey {
        content: source.digest(),
        recipe: *recipe.finalize().as_bytes(),
        frame_index: u64::try_from(request.image_index).map_err(|_| crate::cache::CacheError::Invalid)?,
    };
    if let Some(fingerprint) = cache.get(&key) {
        source.verify(cancelled)?;
        verify_native()?;
        return Ok(CachedFingerprint {
            fingerprint: fingerprint.clone(),
            cache_hit: true,
        });
    }
    let raster = decode_snapshot_frame(request, &source, policy, budget, cancelled)?;
    let fingerprint = raster.view(cancelled)?.fingerprint(cancelled)?;
    source.verify(cancelled)?;
    verify_native()?;
    cache.insert(key, fingerprint.clone(), policy.max_cache_entries)?;
    Ok(CachedFingerprint {
        fingerprint,
        cache_hit: false,
    })
}

/// Decode a selected frame using already validated content and explicit limits.
/// TIFF is identified by magic, independent of extension, and applies per-page ICC.
///
/// # Errors
/// Returns source, directory, selection, color, memory or cancellation errors.
pub(crate) fn decode_snapshot_frame(
    request: &DecodeRequest,
    source: &DecodeSourceSnapshot,
    policy: FingerprintPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<NormalizedRaster, CachedError> {
    let magic = source.read_prefix(4, &cancel)?;
    let mut classified = request.clone();
    classified.memory_budget = Some(budget.clone());
    let sensor = rrrah_decode::image_source_kind(&classified).map_err(FileError::from)?
        == rrrah_decode::ImageSourceKind::Sensor;
    let raster = if !sensor
        && matches!(
            magic.as_slice(),
            [b'I', b'I', 42 | 43, 0] | [b'M', b'M', 0, 42 | 43]
        ) {
        crate::pages::decode_tiff_selected(
            request,
            crate::animated::AnimationBudget {
                max_frames: policy.max_frames,
                max_pixels: policy.max_pixels,
                max_file_bytes: policy.max_file_bytes,
            },
            budget,
            &cancel,
        )?
    } else {
        decode_native_frame(request, policy.max_pixels, budget, &cancel)?
    };
    source.verify(&cancel)?;
    Ok(raster)
}
