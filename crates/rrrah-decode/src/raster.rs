//! Still-image raster decoding. Animation and page selection are separate
//! contracts. DCX selects pages and WAD3 selects textures; other formats return the first image.

use std::{
    io::{Cursor, Read},
    path::Path,
    sync::Arc,
};

use image::{ColorType, DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use thiserror::Error;

use crate::{DecodeError, DecodeRequest, bounded_io::read_bounded};

#[path = "ico_pages.rs"]
mod ico_pages;
#[path = "tiff_alpha.rs"]
mod tiff_alpha;
#[path = "tiff_pages.rs"]
mod tiff_pages;

pub(crate) const MAX_RASTER_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum RasterDecodeError {
    #[error("invalid QOI: {0}")]
    InvalidQoi(&'static str),
    #[error(transparent)]
    Eps(#[from] crate::EpsDocumentError),
    #[error("invalid or unsupported JPEG color: {0}")]
    InvalidJpegColor(&'static str),
    #[error("invalid or unsupported TIFF alpha: {0}")]
    InvalidTiffAlpha(&'static str),
    #[error("invalid TIFF page directory: {0}")]
    InvalidTiffDirectory(&'static str),
    #[error("invalid ICO directory: {0}")]
    InvalidIconDirectory(&'static str),
    #[error(transparent)]
    X3f(#[from] crate::X3fError),
    #[error("invalid or unsupported PICT: {0}")]
    InvalidPict(String),
    #[error("invalid or unsupported PDF: {0}")]
    InvalidPdf(&'static str),
    #[error("invalid or unsupported Basis: {0}")]
    InvalidBasis(&'static str),
    #[error("invalid or unsupported EMF: {0}")]
    InvalidEmf(&'static str),
    #[error("invalid or unsupported MNG: {0}")]
    InvalidMng(&'static str),
    #[error("invalid STI: {0}")]
    InvalidSti(&'static str),
    #[error("invalid or unsupported WMF: {0}")]
    InvalidWmf(&'static str),
    #[error("invalid or unsupported Aseprite: {0}")]
    InvalidAseprite(&'static str),
    #[error("invalid or unsupported JPEG-XR: {0}")]
    InvalidJpegXr(&'static str),
    #[error(transparent)]
    JpegXrCodec(#[from] jpegxr::JXRError),
    #[error("invalid or unsupported JPEG-LS: {0}")]
    InvalidJpegLs(&'static str),
    #[error(transparent)]
    JpegLsCodec(#[from] charls::Error),
    #[error("invalid or unsupported JNG: {0}")]
    InvalidJng(&'static str),
    #[error("invalid or unsupported WebP animation: {0}")]
    InvalidWebpAnimation(&'static str),
    #[error("invalid or unsupported GIF: {0}")]
    InvalidGif(&'static str),
    #[error(transparent)]
    Color(#[from] crate::RasterColorError),
    #[error("invalid or unsupported APNG: {0}")]
    InvalidApng(&'static str),
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error(transparent)]
    Codec(#[from] image::ImageError),
    #[error(transparent)]
    Frame(#[from] rrrah_core::RasterError),
    #[error("raster output exceeds the {MAX_RASTER_BYTES}-byte limit")]
    OutputTooLarge,
    #[error("invalid or unsupported XCF: {0}")]
    InvalidXcf(String),
    #[error("unsupported raster container")]
    UnsupportedFormat,
    #[error("invalid or unsupported OpenRaster: {0}")]
    InvalidOra(String),
    #[error("invalid or unsupported SVG: {0}")]
    InvalidSvg(String),
    #[error("invalid or unsupported JPEG 2000: {0}")]
    InvalidJp2(String),
    #[error("invalid DCX: {0}")]
    InvalidDcx(&'static str),
    #[error("invalid or unsupported Photoshop composition: {0}")]
    InvalidPsd(&'static str),
    #[error("invalid or unsupported Krita document: {0}")]
    InvalidKra(String),
    #[error("WAL requires an external palette; use decode_wal_with_palette")]
    WalPaletteRequired,
    #[error("invalid WAL texture: {0}")]
    InvalidWal(&'static str),
    #[error("invalid or unsupported MacPaint: {0}")]
    InvalidMacPaint(&'static str),
    #[error("RLA matte requires explicit alpha/color interpretation; use decode_rla_with_interpretation")]
    RlaInterpretationRequired,
    #[error("invalid or unsupported NRRD: {0}")]
    InvalidNrrd(&'static str),
    #[error("invalid or unsupported MRC: {0}")]
    InvalidMrc(&'static str),
    #[error("invalid or unsupported FITS: {0}")]
    InvalidFits(&'static str),
    #[error("invalid or unsupported PVR: {0}")]
    InvalidPvr(&'static str),
    #[error("scalar window requires finite increasing bounds with a finite range")]
    InvalidScalarWindow,
    #[error("scalar window requires a supported scientific array (NRRD, MRC or FITS)")]
    ScalarWindowRequiresScientificArray,
    #[error("invalid or unsupported WAD: {0}")]
    InvalidWad(&'static str),
    #[error("invalid or unsupported Wavefront RLA: {0}")]
    InvalidRla(&'static str),
    #[error("invalid or unsupported Softimage PIC: {0}")]
    InvalidSoftimage(&'static str),
    #[error("invalid or unsupported Cineon: {0}")]
    InvalidCineon(&'static str),
    #[error("invalid or unsupported DPX: {0}")]
    InvalidDpx(&'static str),
    #[error("invalid Alias PIX: {0}")]
    InvalidPix(&'static str),
    #[error("invalid or unsupported IFF bitmap: {0}")]
    InvalidIff(&'static str),
    #[error("invalid or unsupported PKM texture: {0}")]
    InvalidPkm(&'static str),
    #[error("invalid or unsupported ASTC texture: {0}")]
    InvalidAstc(String),
    #[error("invalid or unsupported HEIF image: {0}")]
    InvalidHeif(String),
    #[error("invalid or unsupported KTX2 texture: {0}")]
    InvalidKtx2(&'static str),
    #[error("invalid or unsupported KTX texture: {0}")]
    InvalidKtx(&'static str),
    #[error("invalid CUR image: {0}")]
    InvalidCursor(&'static str),
    #[error("invalid or unsupported XPM: {0}")]
    InvalidXpm(&'static str),
    #[error("invalid XBM image: {0}")]
    InvalidXbm(&'static str),
    #[error("invalid AVIF properties: {0}")]
    InvalidAvif(&'static str),
    #[error("invalid or unsupported DDS image: {0}")]
    InvalidDds(&'static str),
    #[error("invalid or unsupported Sun Raster image: {0}")]
    InvalidSun(&'static str),
    #[error("invalid or unsupported SGI image: {0}")]
    InvalidSgi(&'static str),
    #[error("invalid PFM image: {0}")]
    InvalidPfm(&'static str),
    #[error("invalid or unsupported PCX image: {0}")]
    InvalidPcx(&'static str),
    #[error("invalid or unsupported EXR color metadata: {0}")]
    InvalidExrColor(&'static str),
    #[error("invalid EXR geometry metadata: {0}")]
    InvalidExrGeometry(&'static str),
    #[error("invalid Radiance color metadata: {0}")]
    InvalidHdrColor(&'static str),
    #[error("invalid Farbfeld framing")]
    InvalidFarbfeld,
    #[error("RPF requires explicit channel, alpha and color interpretation")]
    RpfInterpretationRequired,
    #[error("RPF raster import failed: {0:?}")]
    Rpf(crate::RpfRasterReadError),
    #[error("invalid or unsupported DICOM: {0}")]
    InvalidDicom(&'static str),
    #[error("unsupported EXR alpha interpretation: {0}")]
    InvalidExrAlpha(&'static str),
    #[error("invalid PNG color metadata: {0}")]
    InvalidPngColor(&'static str),
    #[error("invalid PNG physical pixel dimensions")]
    InvalidPngPhysicalDimensions,
}

/// Decode the first raster image, retaining sample precision and straight
/// alpha. ICC samples remain in their embedded color space. No tone mapping
/// or color conversion is performed here.
pub fn decode_raster_file(path: impl AsRef<Path>) -> Result<DecodedRaster, RasterDecodeError> {
    decode_raster(&DecodeRequest::new(path.as_ref()))
}

pub fn decode_raster(request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    // A small stack-only framing probe also handles renamed EPS. Keep the
    // existing unavailable-file/index error precedence on failed probes.
    let (eps_candidate, rpf_candidate) = std::fs::File::open(&request.path)
        .ok()
        .and_then(|mut file| {
            let mut prefix = [0u8; 740];
            let mut used = 0;
            while used < prefix.len() {
                match file.read(&mut prefix[used..]) {
                    Ok(0) => break,
                    Ok(n) => used += n,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return None,
                }
            }
            Some((
                crate::eps::has_magic(&prefix[..used]),
                crate::rpf::has_magic(&prefix[..used]),
            ))
        })
        .unwrap_or((false, false));
    decode_raster_inspected(request, eps_candidate, rpf_candidate)
}

/// Reuse the router's bounded header probe instead of opening the source again.
pub(crate) fn decode_raster_inspected(
    request: &DecodeRequest,
    eps_candidate: bool,
    rpf_candidate: bool,
) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if !eps_candidate
        && (rpf_candidate
            || request
                .path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("rpf")))
    {
        let interpretation = request
            .rpf_interpretation
            .as_ref()
            .ok_or(RasterDecodeError::RpfInterpretationRequired)?;
        let fallback = rrrah_core::MemoryBudget::new(MAX_RASTER_BYTES);
        let budget = request.memory_budget.as_ref().unwrap_or(&fallback);
        let limits = crate::RpfDecodeLimits {
            max_node_names: 65536,
            max_node_name_bytes: 1024 * 1024,
            max_row_layers: 1024 * 1024,
            max_total_layers: 16 * 1024 * 1024,
            max_output_bytes: MAX_RASTER_BYTES,
        };
        return crate::decode_rpf_file_raster_with_budget(request, budget, limits, interpretation)
            .map(|(raster, _)| raster)
            .map_err(RasterDecodeError::Rpf);
    }
    if eps_candidate {
        let fallback = rrrah_core::MemoryBudget::new(MAX_RASTER_BYTES);
        let budget = request.memory_budget.as_ref().unwrap_or(&fallback);
        return crate::decode_eps_file_document(
            request,
            1.,
            crate::EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
            crate::EpsDocumentLimits::default(),
            budget,
        )
        .map_err(RasterDecodeError::Eps);
    }
    if request.image_index != 0
        && !request.path.extension().is_some_and(|e| {
            e.eq_ignore_ascii_case("cur")
                || e.eq_ignore_ascii_case("ico")
                || e.eq_ignore_ascii_case("tif")
                || e.eq_ignore_ascii_case("tiff")
                || e.eq_ignore_ascii_case("dcm")
                || e.eq_ignore_ascii_case("dicom")
                || e.eq_ignore_ascii_case("ase")
                || e.eq_ignore_ascii_case("aseprite")
                || e.eq_ignore_ascii_case("webp")
                || e.eq_ignore_ascii_case("gif")
                || e.eq_ignore_ascii_case("dcx")
                || e.eq_ignore_ascii_case("wad")
                || e.eq_ignore_ascii_case("nrrd")
                || e.eq_ignore_ascii_case("mrc")
                || e.eq_ignore_ascii_case("mrcs")
                || e.eq_ignore_ascii_case("map")
                || e.eq_ignore_ascii_case("pvr")
                || e.eq_ignore_ascii_case("dds")
                || e.eq_ignore_ascii_case("ktx")
                || e.eq_ignore_ascii_case("basis")
                || e.eq_ignore_ascii_case("pdf")
                || e.eq_ignore_ascii_case("ktx2")
                || e.eq_ignore_ascii_case("apng")
                || e.eq_ignore_ascii_case("mng")
                || e.eq_ignore_ascii_case("sti")
                || e.eq_ignore_ascii_case("wmf")
                || e.eq_ignore_ascii_case("emf")
                || e.eq_ignore_ascii_case("fits")
                || e.eq_ignore_ascii_case("fit")
                || e.eq_ignore_ascii_case("fts")
        })
    {
        // Preserve early unsupported-index errors for unavailable still files,
        // while admitting selectable containers under misleading extensions.
        let mut prefix = Vec::new();
        let selectable = std::fs::File::open(&request.path)
            .and_then(|file| file.take(216).read_to_end(&mut prefix))
            .is_ok()
            && (prefix.starts_with(&[0, 0, 2, 0])
                || ico_pages::has_magic(&prefix)
                || tiff_pages::has_magic(&prefix)
                || crate::dicom::has_magic(&prefix)
                || crate::aseprite::has_magic(&prefix)
                || crate::webp_animation::has_animation(&prefix)
                || crate::gif_animation::has_magic(&prefix)
                || crate::mng::has_magic(&prefix)
                || crate::sti::has_magic(&prefix)
                || crate::wmf::has_magic(&prefix)
                || crate::emf::has_magic(&prefix)
                || crate::dcx::has_magic(&prefix)
                || crate::wad::has_magic(&prefix)
                || crate::nrrd::has_magic(&prefix)
                || crate::mrc::has_magic(&prefix)
                || crate::fits::has_magic(&prefix)
                || crate::pvr::has_magic(&prefix)
                || prefix.starts_with(b"DDS ")
                || crate::ktx::has_magic(&prefix)
                || crate::basis::has_magic(&prefix)
                || crate::pdf::has_magic(&prefix)
                || crate::ktx2::has_magic(&prefix)
                || prefix.starts_with(b"\x89PNG\r\n\x1a\n"));
        if !selectable {
            return Err(DecodeError::UnsupportedImageIndex {
                index: request.image_index,
            }
            .into());
        }
    }
    decode_raster_bytes(read_bounded(request)?, request)
}

pub(crate) fn decode_raster_bytes(
    mut source: impl AsMut<[u8]>,
    request: &DecodeRequest,
) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let bytes = source.as_mut();
    let is_png = bytes.starts_with(b"\x89PNG\r\n\x1a\n");
    // DICOM preambles are arbitrary bytes and may themselves begin with TIFF.
    let cursor = crate::cursor::valid_directory(bytes)
        || (bytes.starts_with(&[0, 0, 2, 0])
            && request
                .path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("cur")));
    let mut raster = if (ico_pages::has_magic(bytes) || cursor) && !crate::dicom::has_magic(bytes) {
        let (mut entry, count) = ico_pages::selected_entry(bytes, request)?;
        let hotspot = if cursor {
            let point = (
                u32::from(u16::from_le_bytes([entry[4], entry[5]])),
                u32::from(u16::from_le_bytes([entry[6], entry[7]])),
            );
            entry[4..8].fill(0);
            Some(point)
        } else {
            None
        };
        let original: [u8; 22] = bytes[..22].try_into().unwrap();
        bytes[2..4].copy_from_slice(&1u16.to_le_bytes());
        bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
        bytes[6..22].copy_from_slice(&entry);
        let mut selected_request = request.clone();
        selected_request.image_index = 0;
        let result = decode_raster_bytes_inner(&mut *bytes, &selected_request);
        bytes[..22].copy_from_slice(&original);
        result?
            .with_image_selection(request.image_index, count)?
            .with_hotspot(hotspot)?
    } else if tiff_pages::has_magic(bytes) && !crate::dicom::has_magic(bytes) {
        let (at, size, selected, count) = tiff_pages::selected_pointer(bytes, request)?;
        let mut original = [0; 8];
        original[..size].copy_from_slice(&bytes[at..at + size]);
        bytes[at..at + size].copy_from_slice(&selected[..size]);
        let mut page_request = request.clone();
        page_request.image_index = 0;
        let result = decode_raster_bytes_inner(&mut *bytes, &page_request);
        bytes[at..at + size].copy_from_slice(&original[..size]);
        result?.with_image_selection(request.image_index, count)?
    } else {
        decode_raster_bytes_inner(bytes, request)?
    };
    // PNG carries the original narrow-range RGB codes. Unlike video codecs
    // that already expand YUV into normalized RGB, it needs an additional
    // range expansion before the existing normalized HDR policy conversion.
    if is_png
        && ((request.pq_reference_white_nits.is_some()
            && matches!(
                raster.color_space(),
                RasterColorSpace::Cicp {
                    primaries: 9,
                    transfer: 16,
                    full_range: false,
                    ..
                }
            ))
            || (request.hlg_display.is_some()
                && matches!(
                    raster.color_space(),
                    RasterColorSpace::Cicp {
                        primaries: 9,
                        transfer: 18,
                        full_range: false,
                        ..
                    }
                )))
    {
        return Err(rrrah_core::RasterError::ColorTransformRequired.into());
    }
    if let Some(white) = request.pq_reference_white_nits
        && matches!(
            raster.color_space(),
            rrrah_core::RasterColorSpace::Cicp {
                primaries: 9,
                transfer: 16,
                ..
            }
        )
    {
        raster =
            raster.pq_to_linear_srgb_with_budget_and_cancel(white, request.memory_budget.as_ref(), || {
                request.check_cancelled().is_err()
            })?;
    }
    if let Some((white, peak, gamma)) = request.hlg_display
        && matches!(
            raster.color_space(),
            rrrah_core::RasterColorSpace::Cicp {
                primaries: 9,
                transfer: 18,
                ..
            }
        )
    {
        raster = raster.hlg_to_linear_srgb_with_budget_and_cancel(
            white,
            peak,
            gamma,
            request.memory_budget.as_ref(),
            || request.check_cancelled().is_err(),
        )?;
    }
    request.check_cancelled()?;
    match &request.memory_budget {
        Some(budget) => raster
            .try_manage_pixels(budget)
            .map_err(|error| RasterDecodeError::Source(DecodeError::Memory(error))),
        None => Ok(raster),
    }
}

// Output admission here is after codec allocation. It bounds retained frames,
// not codec scratch or the transient peak during decoding.
fn decode_raster_bytes_inner(
    mut source: impl AsMut<[u8]>,
    request: &DecodeRequest,
) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let mut bytes = source.as_mut();
    if bytes.starts_with(b"farbfeld") {
        return decode_farbfeld(&bytes, request);
    }
    if crate::xcf::has_magic(&bytes) {
        return crate::xcf::decode_raster(&bytes, request);
    }
    if crate::dicom::has_magic(&bytes) {
        return crate::dicom::decode(&bytes, request, None);
    }

    if crate::aseprite::has_magic(&bytes) {
        return crate::aseprite::decode(&bytes, request).map(|image| image.raster);
    }
    if crate::pvr::has_magic(&bytes) {
        return crate::pvr::decode(&bytes, request).map(|image| image.raster);
    }
    if crate::fits::has_magic(&bytes) {
        return crate::fits::decode_raster(&bytes, request);
    }
    if crate::mrc::has_magic(&bytes) {
        return crate::mrc::decode(&bytes, request).map(|image| image.raster);
    }
    if crate::nrrd::has_magic(&bytes) {
        return crate::nrrd::decode(&bytes, request).map(|image| image.raster);
    }
    if crate::wad::has_magic(&bytes) {
        return crate::wad::decode(&bytes, request);
    }
    if crate::dcx::has_magic(&bytes) {
        return crate::dcx::decode(&bytes, request);
    }
    if bytes.starts_with(b"DDS ") {
        return crate::dds::decode(&bytes, request);
    }
    if crate::ktx::has_magic(&bytes) {
        return crate::ktx::decode(&bytes, request);
    }
    if crate::pdf::has_magic(&bytes) {
        return crate::pdf::decode(&bytes, request);
    }
    if crate::basis::has_magic(&bytes) {
        return crate::basis::decode(&bytes, request);
    }
    if crate::ktx2::has_magic(&bytes) {
        return crate::ktx2::decode(&bytes, request);
    }
    if crate::webp_animation::has_animation(&bytes) {
        return crate::webp_animation::decode(&bytes, request).map(|image| image.raster);
    }
    if crate::gif_animation::has_magic(&bytes) {
        return crate::gif_animation::decode(&bytes, request).map(|image| image.raster);
    }
    if crate::mng::has_magic(&bytes) {
        return crate::mng::decode(&bytes, request).map(|image| image.raster);
    }
    if crate::sti::has_magic(&bytes) {
        return crate::sti::decode(&bytes, request);
    }
    if crate::emf::has_magic(&bytes) {
        return crate::emf::decode(&bytes, request);
    }
    if bytes.starts_with(b"FOVb") {
        request.check_cancelled()?;
        if request.image_index != 0 {
            return Err(DecodeError::UnsupportedImageIndex {
                index: request.image_index,
            }
            .into());
        }
        let fallback = rrrah_core::MemoryBudget::new(MAX_RASTER_BYTES);
        let budget = request.memory_budget.as_ref().unwrap_or(&fallback);
        let result = crate::decode_x3f_legacy(&bytes, budget, || request.check_cancelled().is_err());
        request.check_cancelled()?;
        return result.map_err(Into::into);
    }
    if crate::pict::has_magic(&bytes)
        || request
            .path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pict") || e.eq_ignore_ascii_case("pct"))
    {
        return crate::pict::decode(&bytes, request);
    }
    if crate::wmf::has_magic(&bytes) {
        return crate::wmf::decode(&bytes, request);
    }
    if crate::apng::has_animation(&bytes) {
        return crate::apng::decode(&bytes, request).map(|image| image.raster);
    }
    if request.image_index != 0 {
        return Err(DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    if crate::jpegxr::has_magic(&bytes) {
        return crate::jpegxr::decode(&bytes, request);
    }
    if crate::jpegls::has_magic(&bytes) {
        return crate::jpegls::decode(&bytes, request).map(|image| image.raster);
    }
    if crate::jng::has_magic(&bytes) {
        return crate::jng::decode(&bytes, request);
    }
    if crate::softimage::has_magic(&bytes) {
        return crate::softimage::decode(&bytes, request);
    }
    if crate::cineon::has_magic(&bytes) {
        return crate::cineon::decode(&bytes, request);
    }
    if crate::dpx::has_magic(&bytes) {
        return crate::dpx::decode(&bytes, request);
    }
    if crate::astc::has_magic(&bytes) {
        return crate::astc::decode(&bytes, request);
    }
    if crate::heif::has_magic(&bytes) {
        return crate::heif::decode(&bytes, request);
    }
    if crate::pkm::has_magic(&bytes) {
        return crate::pkm::decode(&bytes, request);
    }
    if crate::iff::has_magic(&bytes) {
        return crate::iff::decode(&bytes, request);
    }
    if crate::psd::has_magic(&bytes) {
        return crate::psd::decode(&bytes, request);
    }
    if crate::jp2::has_magic(&bytes) {
        return crate::jp2::decode(&bytes, request);
    }
    if crate::jxl::has_magic(&bytes) {
        return crate::jxl::decode(&bytes, request);
    }
    if crate::kra::has_magic(&bytes)
        || bytes.starts_with(b"PK\x03\x04")
            && request
                .path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("kra"))
    {
        return crate::kra::decode(&bytes, request);
    }
    if crate::ora::has_magic(&bytes)
        || bytes.starts_with(b"PK\x03\x04")
            && request
                .path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("ora"))
    {
        return crate::ora::decode(&bytes, request);
    }
    if request
        .path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
        && image::guess_format(&bytes).is_err()
    {
        return crate::svg::decode(&bytes, request);
    }
    let hotspot = crate::cursor::normalize(
        &mut bytes,
        request
            .path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cur")),
    )?;

    if bytes.starts_with(b"/* XPM */") || bytes.starts_with(b"! XPM2") {
        return crate::xpm::decode(&bytes, request);
    }
    let avif_properties = crate::avif_color::properties(&bytes)?;
    // The container codec rejects essential clap despite our own handling.
    // Clear only already parsed primary clap associations in the borrowed copy;
    // the source and all unknown essential properties remain untouched.
    for &offset in &avif_properties.handled_aperture_associations {
        bytes[offset] &= 0x7f;
    }
    let png_color = crate::png_color::declaration(&bytes)?;
    let exr_metadata = crate::exr_color::metadata(&bytes)?;
    let hdr_color = crate::hdr_color::declaration(&bytes)?;
    // QOI validates framing while decoding under the output reservation.
    if bytes.starts_with(b"qoif") {
        return crate::qoi_bounds::decode(&bytes, request);
    }
    if bytes.starts_with(b"DDS ") {
        return crate::dds::decode(&bytes, request);
    }
    if bytes.starts_with(&[0x59, 0xa6, 0x6a, 0x95]) {
        return crate::sun::decode(&bytes, request);
    }
    if bytes.starts_with(&[1, 218]) {
        return crate::sgi::decode(&bytes, request);
    }
    if bytes.starts_with(b"PF\n")
        || bytes.starts_with(b"Pf\n")
        || bytes.starts_with(b"PF\r\n")
        || bytes.starts_with(b"Pf\r\n")
    {
        return crate::pfm::decode(&bytes, request);
    }
    if crate::macpaint::has_wrapper(&bytes)
        || (crate::macpaint::is_path(&request.path) && image::guess_format(&bytes).is_err())
    {
        return crate::macpaint::decode(&bytes, request);
    }
    if request
        .path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("rla"))
        && image::guess_format(&bytes).is_err()
    {
        return crate::rla::decode(&bytes, request);
    }
    if request
        .path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("wal"))
        && image::guess_format(&bytes).is_err()
    {
        return crate::wal::decode_using_game_palette(&bytes, request);
    }
    if request
        .path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pix"))
        && image::guess_format(&bytes).is_err()
    {
        return crate::pix::decode(&bytes, request);
    }
    if bytes.first() == Some(&0x0a) {
        return crate::pcx::decode(&bytes, request);
    }
    if image::guess_format(&bytes).is_err()
        && request
            .path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("xbm"))
    {
        return crate::xbm::decode(&bytes, request);
    }
    // image's format guesser recognizes classic TIFF only. Admit BigTIFF from
    // its explicit header; the TIFF decoder validates offsets and sample data.
    let big_tiff = bytes.starts_with(b"II+\0\x08\0\0\0") || bytes.starts_with(b"MM\0+\0\x08\0\0");
    let mut reader = ImageReader::new(Cursor::new(&*bytes))
        .with_guessed_format()
        .map_err(|source| DecodeError::Io {
            path: request.path.clone(),
            source,
        })?;
    if reader.format().is_none() && big_tiff {
        reader.set_format(ImageFormat::Tiff);
    }
    // TGA has no strong leading magic; use its extension only when sniffing
    // found no known container. Recognized contents always win.
    if reader.format().is_none()
        && request
            .path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("tga"))
    {
        reader.set_format(ImageFormat::Tga);
    }
    let format = reader.format().ok_or(RasterDecodeError::UnsupportedFormat)?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_RASTER_BYTES);
    limits.max_image_width = Some(65536);
    limits.max_image_height = Some(65536);
    reader.limits(limits.clone());
    // image's TIFF limits tie tag-value allocation to pixel-buffer size. Small
    // images can therefore lose a larger ICC tag via its swallowed limits error.
    // Read metadata under the TIFF decoder's bounded defaults before applying
    // pixel limits; retain those limits for actual image decoding.
    let (mut decoder, tiff_profile): (Box<dyn ImageDecoder + '_>, _) = if format == ImageFormat::Tiff {
        let mut decoder = image::codecs::tiff::TiffDecoder::new(reader.into_inner())?;
        let profile = decoder.icc_profile()?;
        decoder.set_limits(limits)?;
        (Box::new(decoder), Some(profile))
    } else {
        (Box::new(reader.into_decoder()?), None)
    };
    let associated_tiff = format == ImageFormat::Tiff && tiff_alpha::associated(bytes, request)?;
    if associated_tiff
        && !matches!(
            decoder.color_type(),
            ColorType::La8 | ColorType::La16 | ColorType::Rgba8 | ColorType::Rgba16 | ColorType::Rgba32F
        )
    {
        return Err(RasterDecodeError::InvalidTiffAlpha("missing alpha channel"));
    }
    let (width, height) = decoder.dimensions();
    let clean_crop = avif_properties
        .clean_aperture
        .as_ref()
        .map(|data| crate::avif_color::crop_rect(data, width, height))
        .transpose()?;
    let sample_bytes = match decoder.color_type() {
        ColorType::L16 | ColorType::La16 | ColorType::Rgb16 | ColorType::Rgba16 => 2,
        ColorType::Rgb32F | ColorType::Rgba32F => 4,
        _ => 1,
    };
    if u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4 * sample_bytes)
        > MAX_RASTER_BYTES
    {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    // Admit final RGBA capacity before the codec allocates its decoded image.
    // Codec scratch and overlapping conversion/orientation buffers are separate.
    let (output_width, output_height) = clean_crop.map_or((width, height), |rect| (rect[2], rect[3]));
    let output_bytes = u64::from(output_width) * u64::from(output_height) * 4 * sample_bytes;
    let output_reservation = request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve(output_bytes))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let profile = match tiff_profile {
        Some(profile) => profile,
        None => decoder.icc_profile()?,
    };
    let orientation = decoder.orientation()?;
    request.check_cancelled()?;
    let cmyk = if format == ImageFormat::Jpeg {
        crate::jpeg_cmyk::decode_profiled(&bytes, profile.as_deref(), width, height, request)?
    } else {
        None
    };
    let cmyk_converted = cmyk.is_some();
    let mut image = match cmyk {
        Some(image) => image,
        None => DynamicImage::from_decoder(decoder)?,
    };
    if let Some([x, y, width, height]) = clean_crop {
        image = image.crop_imm(x, y, width, height);
    }
    let pixel_aspect = png_color
        .pixel_aspect
        .or(exr_metadata.pixel_aspect)
        .map(|aspect| {
            if orientation.to_exif() >= 5 {
                1.0 / aspect
            } else {
                aspect
            }
        });
    image.apply_orientation(orientation);
    // HEIF transforms apply rotation (counterclockwise), then mirroring.
    image = match avif_properties.rotation {
        1 => image.rotate270(),
        2 => image.rotate180(),
        3 => image.rotate90(),
        _ => image,
    };
    image = match avif_properties.mirror {
        Some(0) => image.flipv(),
        Some(1) => image.fliph(),
        _ => image,
    };
    request.check_cancelled()?;
    let color_space = if cmyk_converted {
        RasterColorSpace::Srgb
    } else if png_color.override_icc {
        png_color.color.unwrap_or(RasterColorSpace::Unspecified)
    } else {
        profile.map_or_else(
            || {
                png_color
                    .color
                    .or(avif_properties.color)
                    .or(exr_metadata.color)
                    .or(hdr_color)
                    .unwrap_or(match format {
                        ImageFormat::Hdr => RasterColorSpace::LinearRgbUnspecified,
                        ImageFormat::Farbfeld => RasterColorSpace::AssumedSrgb,
                        ImageFormat::Jpeg
                        | ImageFormat::Gif
                        | ImageFormat::WebP
                        | ImageFormat::Bmp
                        | ImageFormat::Ico => RasterColorSpace::Srgb,
                        ImageFormat::Tiff if sample_bytes == 4 && request.assume_untagged_linear_srgb => {
                            RasterColorSpace::LinearSrgb
                        }
                        ImageFormat::Tiff | ImageFormat::Pnm | ImageFormat::Tga
                            if request.assume_untagged_srgb =>
                        {
                            RasterColorSpace::AssumedSrgb
                        }
                        // Standard PBM/PGM/PPM use full-range BT.709, not sRGB.
                        // Nonstandard sRGB variants require the explicit override;
                        // PAM tuple/color interpretation remains separate.
                        ImageFormat::Pnm
                            if matches!(
                                bytes.get(..2),
                                Some(b"P1" | b"P2" | b"P3" | b"P4" | b"P5" | b"P6")
                            ) =>
                        {
                            RasterColorSpace::Bt709
                        }
                        // EXR/HDR primaries, TIFF/PNG/TGA gamma and QOI's transfer flag
                        // require metadata handling before a display transform is chosen.
                        _ => RasterColorSpace::Unspecified,
                    })
            },
            RasterColorSpace::Icc,
        )
    };
    let color_space = request.qualify_linear_color(color_space);
    let (width, height) = (image.width(), image.height());
    let pixels = match sample_bytes {
        2 => {
            let mut pixels = image.into_rgba16().into_raw();
            if format == ImageFormat::Avif {
                let depth = avif_properties.bit_depth.ok_or(RasterDecodeError::InvalidAvif(
                    "missing primary AV1 configuration",
                ))?;
                if avif_properties.premultiplied_alpha {
                    crate::avif_color::unassociate_u16(&mut pixels, depth, request)?;
                } else {
                    normalize_avif_u16(&mut pixels, depth, request)?;
                }
            }
            if associated_tiff {
                tiff_alpha::u16_pixels(&mut pixels, request)?;
            }
            RasterPixels::Rgba16(adopt_raster_output(pixels, output_reservation)?)
        }
        4 => {
            let mut pixels = image.into_rgba32f().into_raw();
            if associated_tiff {
                tiff_alpha::f32_pixels(&mut pixels, request)?;
            }
            if format == ImageFormat::OpenExr {
                // OpenEXR's conventional RGB channels are alpha-associated.
                // Our raster API and common renderer use straight alpha.
                for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
                    if index % 4096 == 0 {
                        request.check_cancelled()?;
                    }
                    let alpha = pixel[3];
                    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
                        return Err(RasterDecodeError::InvalidExrAlpha(
                            "non-finite or out-of-range alpha",
                        ));
                    }
                    if alpha == 0.0 {
                        if pixel[..3].iter().any(|value| *value != 0.0) {
                            return Err(RasterDecodeError::InvalidExrAlpha(
                                "zero-alpha emission requires associated-alpha rendering",
                            ));
                        }
                    } else {
                        for value in &mut pixel[..3] {
                            *value /= alpha;
                            if !value.is_finite() {
                                return Err(RasterDecodeError::InvalidExrAlpha(
                                    "unassociation overflows or contains non-finite RGB",
                                ));
                            }
                        }
                    }
                }
            }
            RasterPixels::Rgba32Float(adopt_raster_output(pixels, output_reservation)?)
        }
        _ => {
            let mut pixels = image.into_rgba8().into_raw();
            if format == ImageFormat::Avif && avif_properties.premultiplied_alpha {
                crate::avif_color::unassociate_u8(&mut pixels, request)?;
            }
            if associated_tiff {
                tiff_alpha::u8_pixels(&mut pixels, request)?;
            }
            RasterPixels::Rgba8(adopt_raster_output(pixels, output_reservation)?)
        }
    };
    Ok(DecodedRaster::new(width, height, pixels, color_space)?
        .with_pixel_aspect(pixel_aspect)?
        .with_hotspot(hotspot)?)
}

// image 0.25.10 expands AVIF depth with a bit rotation, rather than scaling
// the integer code range. Restore codes before normalizing into full u16.
fn normalize_avif_u16(
    pixels: &mut [u16],
    depth: u8,
    request: &DecodeRequest,
) -> Result<(), RasterDecodeError> {
    if !matches!(depth, 10 | 12) {
        return Err(RasterDecodeError::InvalidAvif(
            "16-bit output with invalid coded depth",
        ));
    }
    let shift = 16 - depth;
    let maximum = (1u32 << depth) - 1;
    let mask = (1u16 << shift) - 1;
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        if index % 4096 == 0 {
            request.check_cancelled()?;
        }
        for (channel, sample) in pixel.iter_mut().enumerate() {
            if channel == 3 && *sample == u16::MAX {
                continue;
            }
            if *sample & mask != 0 {
                return Err(RasterDecodeError::InvalidAvif(
                    "codec samples disagree with coded depth",
                ));
            }
            let code = u32::from(*sample >> shift);
            *sample = ((code * 65535 + maximum / 2) / maximum) as u16;
        }
    }
    Ok(())
}

fn decode_farbfeld(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    if request.image_index != 0 {
        return Err(DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let invalid = || RasterDecodeError::InvalidFarbfeld;
    let header = bytes.get(..16).ok_or_else(invalid)?;
    let width = u32::from_be_bytes(header[8..12].try_into().unwrap());
    let height = u32::from_be_bytes(header[12..16].try_into().unwrap());
    let size = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(8))
        .filter(|n| *n > 0 && *n <= MAX_RASTER_BYTES)
        .ok_or(RasterDecodeError::OutputTooLarge)?;
    if u64::try_from(bytes.len()).ok() != size.checked_add(16) {
        return Err(invalid());
    }
    let reservation = request
        .memory_budget
        .as_ref()
        .map(|b| b.try_reserve(size))
        .transpose()
        .map_err(|e| RasterDecodeError::Source(DecodeError::Memory(e)))?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(size as usize / 2)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    for chunk in bytes[16..].chunks(4096 * 8) {
        request.check_cancelled()?;
        values.extend(chunk.chunks_exact(2).map(|v| u16::from_be_bytes([v[0], v[1]])));
    }
    request.check_cancelled()?;
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba16(adopt_raster_output(values, reservation)?),
        RasterColorSpace::AssumedSrgb,
    )?)
}

pub(crate) fn adopt_raster_output<T: Copy>(
    values: Vec<T>,
    reservation: Option<rrrah_core::Reservation>,
) -> Result<rrrah_core::PixelBuffer<T>, RasterDecodeError> {
    match reservation {
        Some(reservation) => reservation
            .try_adopt(values)
            .map(Into::into)
            .map_err(|error| RasterDecodeError::Source(DecodeError::Memory(error))),
        None => Ok(Arc::new(values).into()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn eps_framing_wins_over_raster_raw_and_unknown_suffixes() {
        let code=b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 2 2\n%%EndComments\n1 0 0 setrgbcolor 0 0 2 2 rectfill\n%%EOF\n";
        for suffix in ["eps", "png", "nef", "unknown"] {
            let path = std::env::temp_dir().join(format!(
                "rrrah-eps-magic-{}-{suffix}.{suffix}",
                std::process::id()
            ));
            std::fs::write(&path, code).unwrap();
            let root = rrrah_core::MemoryBudget::new(32 * 1024 * 1024);
            let mut request = crate::DecodeRequest::new(&path);
            request.memory_budget = Some(root.clone());
            assert_eq!(
                crate::image_source_kind(&request).unwrap(),
                crate::ImageSourceKind::Raster
            );
            let crate::DecodedImage::Raster(image) = crate::decode_image(&request).unwrap() else {
                panic!()
            };
            assert_eq!((image.width(), image.height()), (2, 2));
            let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
                panic!()
            };
            assert!(pixels.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
            drop(image);
            assert_eq!(root.used(), 0);
            request.image_index = 1;
            assert!(matches!(
                super::decode_raster(&request),
                Err(super::RasterDecodeError::Eps(_))
            ));
            assert_eq!(root.used(), 0);
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn png_magic_wins_over_eps_suffix() {
        let path = std::env::temp_dir().join(format!(
            "rrrah-png-eps-{}-{}.eps",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/pattern.png"),
        )
        .unwrap();
        let mut request = crate::DecodeRequest::new(&path);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(1024 * 1024));
        let image = super::decode_raster(&request).unwrap();
        assert_eq!((image.width(), image.height()), (16, 16));
        drop(image);
        assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn avif_integer_normalization_covers_every_10_and_12_bit_code() {
        let request = crate::DecodeRequest::new("unused.avif");
        for depth in [10u8, 12] {
            let maximum = (1u16 << depth) - 1;
            let mut pixels = Vec::new();
            for code in 0..=maximum {
                pixels.extend([code << (16 - depth); 4]);
            }
            super::normalize_avif_u16(&mut pixels, depth, &request).unwrap();
            for (code, rgba) in pixels.chunks_exact(4).enumerate() {
                let expected = ((code as f64 / f64::from(maximum)) * 65535.0).round() as u16;
                assert_eq!(rgba, [expected; 4]);
            }
            assert_eq!(pixels[0], 0);
            assert_eq!(*pixels.last().unwrap(), 65535);
            let mut invalid = [1, 0, 0, 0];
            assert!(super::normalize_avif_u16(&mut invalid, depth, &request).is_err());
        }
    }
    #[test]
    fn tiny_tiff_retains_icc_and_applies_orientation_with_managed_output() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/oriented.profiled.tif");
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = crate::DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        let frame = super::decode_raster(&request).unwrap();
        assert_eq!((frame.width(), frame.height()), (1, 2));
        assert!(
            matches!(frame.color_space(), rrrah_core::RasterColorSpace::Icc(profile) if profile.len() == 588)
        );
        let rrrah_core::RasterPixels::Rgba8(pixels) = frame.pixels() else {
            panic!()
        };
        assert!(pixels.is_managed());
        assert_eq!(&**pixels, &[255, 0, 0, 255, 0, 255, 0, 128]);
        assert_eq!(budget.used(), 8 + 588);
        let prepared = crate::prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
        assert_eq!((prepared.width(), prepared.height()), (1, 2));
        let rrrah_core::RasterPixels::Rgba32Float(pixels) = prepared.pixels() else {
            panic!()
        };
        assert_eq!(pixels[3], 1.0);
        assert_eq!(pixels[7], 128.0 / 255.0);
        drop(prepared);
        drop(frame);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn pillow_tiff_retains_embedded_icc_for_display() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/pattern.profiled.tif");
        let raster = super::decode_raster_file(&path).unwrap();
        assert!(
            matches!(raster.color_space(), rrrah_core::RasterColorSpace::Icc(profile) if profile.len() == 588),
            "embedded TIFF color: {:?}",
            raster.color_space()
        );
        crate::prepare_raster_for_display(&raster).unwrap();
        let source_bytes = std::fs::metadata(&path).unwrap().len();
        let pixel_bytes = u64::from(raster.width()) * u64::from(raster.height()) * 4;
        let mut request = crate::DecodeRequest::new(&path);
        let insufficient = rrrah_core::MemoryBudget::new(pixel_bytes + 587);
        let unowned = super::decode_raster_file(&path).unwrap();
        assert!(matches!(
            unowned.try_manage_pixels(&insufficient),
            Err(rrrah_core::BufferError::Capacity { .. })
        ));
        assert_eq!(insufficient.used(), 0);
        let budget = rrrah_core::MemoryBudget::new(source_bytes + pixel_bytes + 588);
        request.memory_budget = Some(budget.clone());
        let managed = super::decode_raster(&request).unwrap();
        assert_eq!(budget.used(), pixel_bytes + 588);
        let held = managed.clone();
        drop(managed);
        assert_eq!(budget.used(), pixel_bytes + 588);
        drop(held);
        assert_eq!(budget.used(), 0);
    }

    use super::*;
    use image::ImageEncoder;

    #[test]
    fn avif_prem_alpha_is_unassociated_before_color_preparation() {
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let plain = decode_raster_file(&fixtures.join("avif-alpha-icc.avif")).unwrap();
        let prem = decode_raster_file(&fixtures.join("avif-alpha-icc-prem.avif")).unwrap();
        assert_eq!(plain.color_space(), prem.color_space());
        let (RasterPixels::Rgba8(a), RasterPixels::Rgba8(b)) = (plain.pixels(), prem.pixels()) else {
            panic!("expected RGBA8");
        };
        assert_ne!(&a[..], &b[..]);
        for (source, recovered) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
            assert_eq!(source[3], recovered[3]);
            for channel in 0..3 {
                let expected = if source[3] == 0 {
                    0
                } else {
                    (f64::from(source[channel]) * 255.0 / f64::from(source[3]))
                        .round()
                        .min(255.0) as u8
                };
                assert_eq!(recovered[channel], expected);
            }
        }
        let expected_bytes: Vec<u8> = a
            .chunks_exact(4)
            .flat_map(|pixel| {
                let mut output = [0u8; 4];
                output[3] = pixel[3];
                for channel in 0..3 {
                    output[channel] = if pixel[3] == 0 {
                        0
                    } else {
                        (f64::from(pixel[channel]) * 255.0 / f64::from(pixel[3]))
                            .round()
                            .min(255.0) as u8
                    };
                }
                output
            })
            .collect();
        let expected = DecodedRaster::new(
            plain.width(),
            plain.height(),
            RasterPixels::Rgba8(std::sync::Arc::new(expected_bytes).into()),
            plain.color_space().clone(),
        )
        .unwrap();
        let expected = crate::prepare_raster_for_display(&expected).unwrap();
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = crate::DecodeRequest::new(fixtures.join("avif-alpha-icc-prem.avif"));
        request.memory_budget = Some(budget.clone());
        let managed = decode_raster(&request).unwrap();
        let prepared = crate::prepare_raster_for_display_with_budget(&managed, Some(&budget)).unwrap();
        let (RasterPixels::Rgba32Float(actual), RasterPixels::Rgba32Float(expected)) =
            (prepared.pixels(), expected.pixels())
        else {
            panic!("linear preparation");
        };
        assert_eq!(actual.len(), expected.len());
        assert!(
            actual
                .iter()
                .zip(expected.iter())
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        drop(prepared);
        drop(managed);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn avif_prem_conversion_cancellation_preserves_input_and_releases_budget() {
        use std::sync::{Arc, atomic::AtomicU64};
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = crate::DecodeRequest::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/raster/avif-alpha-icc-prem.avif"),
        );
        request.memory_budget = Some(budget.clone());
        request.cancellation = Some(crate::GenerationToken::new(Arc::new(AtomicU64::new(1)), 0));
        let mut bytes = [17, 0, 18, 17];
        assert!(matches!(
            crate::avif_color::unassociate_u8(&mut bytes, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!(bytes, [17, 0, 18, 17]);
        let mut words = [64, 128, 192, 640];
        assert!(matches!(
            crate::avif_color::unassociate_u16(&mut words, 10, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!(words, [64, 128, 192, 640]);
        assert!(matches!(
            decode_raster(&request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn avif_square_pixel_property_preserves_samples_and_rejects_invalid_geometry() {
        let fixtures =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let reference = decode_raster_file(&fixtures.join("avif-rgb.avif")).unwrap();
        let reference_prepared = crate::prepare_raster_for_display(&reference).unwrap();
        for (name, valid) in [
            ("avif-pasp-square.avif", true),
            ("avif-pasp-square-essential.avif", true),
            ("avif-pasp-nonsquare.avif", false),
            ("avif-pasp-zero.avif", false),
        ] {
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            let mut request = crate::DecodeRequest::new(fixtures.join(name));
            request.memory_budget = Some(budget.clone());
            let result = decode_raster(&request);
            if valid {
                let decoded = result.unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(
                    (decoded.width(), decoded.height()),
                    (reference.width(), reference.height())
                );
                assert_eq!(decoded.color_space(), reference.color_space());
                let (RasterPixels::Rgba8(actual), RasterPixels::Rgba8(expected)) =
                    (decoded.pixels(), reference.pixels())
                else {
                    panic!("AVIF precision")
                };
                assert_eq!(&actual[..], &expected[..]);
                let prepared =
                    crate::prepare_raster_for_display_with_budget(&decoded, Some(&budget)).unwrap();
                assert_eq!(
                    (prepared.width(), prepared.height()),
                    (reference.width(), reference.height())
                );
                let (RasterPixels::Rgba32Float(actual), RasterPixels::Rgba32Float(expected)) =
                    (prepared.pixels(), reference_prepared.pixels())
                else {
                    panic!("prepared precision")
                };
                assert_eq!(actual.len(), expected.len());
                assert!(
                    actual
                        .iter()
                        .zip(expected.iter())
                        .all(|(a, b)| a.to_bits() == b.to_bits())
                );
                drop(prepared);
                drop(decoded);
            } else {
                assert!(
                    matches!(result, Err(RasterDecodeError::InvalidAvif(_))),
                    "{name}: {result:?}"
                );
            }
            assert_eq!(budget.used(), 0, "{name}");
        }
    }

    #[test]
    fn exr_display_window_crops_and_places_data_in_pixel_coordinates() {
        let base = include_bytes!("../../../tests/fixtures/raster/float-none-rec709-declared.exr");
        let attribute = b"displayWindow\0box2i\0\x10\0\0\0";
        let offset = base
            .windows(attribute.len())
            .position(|v| v == attribute)
            .unwrap()
            + attribute.len();
        let reference = decode_fixture("window-reference.exr", base).unwrap();
        let RasterPixels::Rgba32Float(source) = reference.pixels() else {
            panic!("EXR precision")
        };
        assert_eq!((reference.width(), reference.height()), (3, 2));
        // Display-only changes retain scanlines. Shifted fixtures additionally update
        // scanline Y coordinates, preserving every encoded channel sample.
        let cases: [([i32; 2], [i32; 4], Option<&[u8]>); 8] = [
            ([0, 0], [-1, -1, 3, 2], None),
            ([0, 0], [1, 0, 2, 1], None),
            ([0, 0], [-2, 1, 1, 3], None),
            ([0, 0], [5, 5, 6, 6], None),
            ([0, 0], [0, 0, 2, 1], None),
            (
                [-2, -1],
                [-3, -2, 2, 2],
                Some(include_bytes!(
                    "../../../tests/fixtures/raster/float-shifted-window-negative.exr"
                )),
            ),
            (
                [2, 1],
                [3, 1, 5, 3],
                Some(include_bytes!(
                    "../../../tests/fixtures/raster/float-shifted-window-positive-crop.exr"
                )),
            ),
            (
                [-7, 9],
                [-7, 9, -5, 10],
                Some(include_bytes!(
                    "../../../tests/fixtures/raster/float-shifted-window-translated.exr"
                )),
            ),
        ];
        for (origin, bounds, fixture) in cases {
            let mut bytes = fixture.unwrap_or(base).to_vec();
            for (index, coordinate) in bounds.iter().enumerate() {
                bytes[offset + index * 4..offset + index * 4 + 4].copy_from_slice(&coordinate.to_le_bytes());
            }
            let path = std::env::temp_dir().join(format!(
                "rrrah-exr-window-{}-{}.exr",
                std::process::id(),
                bounds[0]
            ));
            std::fs::write(&path, bytes).unwrap();
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            let mut request = crate::DecodeRequest::new(path.clone());
            request.memory_budget = Some(budget.clone());
            let result = decode_raster(&request);
            std::fs::remove_file(path).unwrap();
            let decoded = result.unwrap();
            assert_eq!(
                (decoded.width(), decoded.height()),
                (
                    (bounds[2] - bounds[0] + 1) as u32,
                    (bounds[3] - bounds[1] + 1) as u32
                )
            );
            let RasterPixels::Rgba32Float(actual) = decoded.pixels() else {
                panic!("EXR precision")
            };
            for y in bounds[1]..=bounds[3] {
                for x in bounds[0]..=bounds[2] {
                    let destination =
                        (((y - bounds[1]) as u32 * decoded.width() + (x - bounds[0]) as u32) * 4) as usize;
                    let expected: &[f32] = if (origin[0]..origin[0] + 3).contains(&x)
                        && (origin[1]..origin[1] + 2).contains(&y)
                    {
                        &source[(((y - origin[1]) * 3 + x - origin[0]) * 4) as usize
                            ..(((y - origin[1]) * 3 + x - origin[0]) * 4 + 4) as usize]
                    } else {
                        &[0.; 4]
                    };
                    for channel in 0..4 {
                        assert_eq!(
                            actual[destination + channel].to_bits(),
                            expected[channel].to_bits(),
                            "bounds={bounds:?} at ({x},{y}) channel={channel}"
                        );
                    }
                }
            }
            let prepared = crate::prepare_raster_for_display_with_budget(&decoded, Some(&budget)).unwrap();
            assert_eq!(
                (prepared.width(), prepared.height()),
                (decoded.width(), decoded.height())
            );
            drop(prepared);
            drop(decoded);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn exr_physical_pixel_aspect_reaches_display_preparation_without_changing_samples() {
        let base = include_bytes!("../../../tests/fixtures/raster/float-none-rec709-declared.exr");
        let attribute = b"pixelAspectRatio\0float\0\x04\0\0\0";
        let offset = base
            .windows(attribute.len())
            .position(|v| v == attribute)
            .unwrap()
            + attribute.len();
        let reference = decode_fixture("aspect-reference.exr", base).unwrap();
        for aspect in [2.0f32, 0.5, 1.0] {
            let mut bytes = base.to_vec();
            bytes[offset..offset + 4].copy_from_slice(&aspect.to_le_bytes());
            let path = std::env::temp_dir().join(format!(
                "rrrah-exr-physical-{}-{}.exr",
                std::process::id(),
                aspect.to_bits()
            ));
            std::fs::write(&path, bytes).unwrap();
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            let mut request = crate::DecodeRequest::new(path.clone());
            request.memory_budget = Some(budget.clone());
            let result = decode_raster(&request);
            std::fs::remove_file(path).unwrap();
            let frame = result.unwrap();
            assert_eq!(frame.pixel_aspect(), Some(aspect));
            assert_eq!(
                (frame.width(), frame.height()),
                (reference.width(), reference.height())
            );
            let (RasterPixels::Rgba32Float(actual), RasterPixels::Rgba32Float(expected)) =
                (frame.pixels(), reference.pixels())
            else {
                panic!("EXR precision");
            };
            assert!(
                actual
                    .iter()
                    .zip(expected.iter())
                    .all(|(a, b)| a.to_bits() == b.to_bits())
            );
            let prepared = crate::prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
            assert_eq!(prepared.pixel_aspect(), Some(aspect));
            drop(prepared);
            drop(frame);
            assert_eq!(budget.used(), 0);
        }
        for aspect in [0.0f32, -1.0, f32::NAN, f32::INFINITY] {
            let mut bytes = base.to_vec();
            bytes[offset..offset + 4].copy_from_slice(&aspect.to_le_bytes());
            let path = std::env::temp_dir().join(format!(
                "rrrah-exr-invalid-aspect-{}-{}.exr",
                std::process::id(),
                aspect.to_bits()
            ));
            std::fs::write(&path, bytes).unwrap();
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            let mut request = crate::DecodeRequest::new(path.clone());
            request.memory_budget = Some(budget.clone());
            let result = decode_raster(&request);
            std::fs::remove_file(path).unwrap();
            assert!(matches!(result, Err(RasterDecodeError::InvalidExrGeometry(_))));
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn png_physical_pixel_aspect_reaches_common_raster_without_changing_samples() {
        let image = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([128, 64, 32, 255]),
        ));
        let mut encoded = Cursor::new(Vec::new());
        image.write_to(&mut encoded, ImageFormat::Png).unwrap();
        let base = encoded.into_inner();
        let reference = decode_fixture("square.png", &base).unwrap();
        for (x, y, unit, expected) in [
            (2u32, 1u32, 0u8, Some(0.5)),
            (1000, 2000, 1, Some(2.)),
            (0, 1, 0, None),
        ] {
            let mut data = x.to_be_bytes().to_vec();
            data.extend_from_slice(&y.to_be_bytes());
            data.push(unit);
            let physical = crate::png_color::tests::chunk(b"pHYs", &data);
            let mut bytes = base[..33].to_vec();
            bytes.extend_from_slice(&physical);
            bytes.extend_from_slice(&base[33..]);
            let frame = decode_fixture("physical.png", &bytes).unwrap();
            assert_eq!(frame.pixel_aspect(), expected);
            let (RasterPixels::Rgba8(actual), RasterPixels::Rgba8(expected_pixels)) =
                (frame.pixels(), reference.pixels())
            else {
                panic!("PNG precision");
            };
            assert_eq!(actual, expected_pixels);
            let exif = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
            let mut rotated = bytes[..33].to_vec();
            rotated.extend_from_slice(&crate::png_color::tests::chunk(b"eXIf", exif));
            rotated.extend_from_slice(&bytes[33..]);
            assert_eq!(
                decode_fixture("rotated.png", &rotated).unwrap().pixel_aspect(),
                expected.map(|v| 1. / v)
            );
            let mut corrupt = bytes.clone();
            corrupt[41] ^= 1;
            assert!(matches!(
                decode_fixture("bad-crc.png", &corrupt),
                Err(RasterDecodeError::InvalidPngPhysicalDimensions)
            ));
            let mut duplicate = bytes[..33].to_vec();
            duplicate.extend_from_slice(&physical);
            duplicate.extend_from_slice(&bytes[33..]);
            assert!(decode_fixture("duplicate.png", &duplicate).is_err());
        }
    }

    #[test]
    fn declared_png_srgb_and_linear_cicp_reach_linear_pixel_conversion() {
        let image = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([128, 128, 128, 64]),
        ));
        let mut encoded = Cursor::new(Vec::new());
        image.write_to(&mut encoded, ImageFormat::Png).unwrap();
        let base = encoded.into_inner();
        for (name, chunk, expected) in [
            (
                "srgb",
                crate::png_color::tests::chunk(b"sRGB", &[0]),
                0.2158605_f32,
            ),
            (
                "linear",
                crate::png_color::tests::chunk(b"cICP", &[1, 8, 0, 1]),
                128.0 / 255.0,
            ),
        ] {
            let mut bytes = base[..33].to_vec();
            bytes.extend_from_slice(&chunk);
            bytes.extend_from_slice(&base[33..]);
            let frame = decode_fixture(&format!("png-{name}.png"), &bytes).unwrap();
            let linear = frame.to_linear_srgb().unwrap();
            let RasterPixels::Rgba32Float(p) = linear.pixels() else {
                panic!("wrong precision")
            };
            assert!((p[0] - expected).abs() < 1e-6);
            assert!((p[3] - 64.0 / 255.0).abs() < 1e-7);
        }
    }

    #[test]
    fn png_gamma_chroma_display_matches_full_u16_power_reference() {
        for (gamma, p3, custom) in [
            (50000u32, false, false),
            (45455, false, false),
            (100000, true, false),
            (50000, true, false),
            (50000, true, true),
        ] {
            let image = DynamicImage::ImageRgba16(image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::from_fn(
                65536,
                1,
                |x, _| {
                    let value = x as u16;
                    image::Rgba(if p3 {
                        [value, 0, 0, 32768]
                    } else {
                        [value, value, value, 32768]
                    })
                },
            ));
            let mut encoded = Cursor::new(Vec::new());
            image.write_to(&mut encoded, ImageFormat::Png).unwrap();
            let base = encoded.into_inner();
            let mut bytes = base[..33].to_vec();
            bytes.extend_from_slice(&crate::png_color::tests::chunk(b"gAMA", &gamma.to_be_bytes()));
            let chrm = if custom {
                let xy: [u32; 8] = [34567, 35850, 67000, 32000, 21000, 71000, 14000, 8000];
                crate::png_color::tests::chunk(
                    b"cHRM",
                    &xy.into_iter().flat_map(u32::to_be_bytes).collect::<Vec<_>>(),
                )
            } else {
                crate::png_color::tests::chrm(p3)
            };
            bytes.extend_from_slice(&chrm);
            bytes.extend_from_slice(&base[33..]);
            let source = decode_fixture(&format!("gamma-chroma-{gamma}-{p3}.png"), &bytes).unwrap();
            let RasterPixels::Rgba16(native) = source.pixels() else {
                panic!()
            };
            for (code, rgba) in native.chunks_exact(4).enumerate() {
                let expected = if p3 {
                    [code as u16, 0, 0, 32768]
                } else {
                    [code as u16, code as u16, code as u16, 32768]
                };
                assert_eq!(rgba, &expected);
            }
            let root = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
            assert!(
                crate::prepare_raster_for_display_with_budget_and_cancel(&source, Some(&root), || root
                    .used()
                    > 0)
                .is_err()
            );
            assert_eq!(root.used(), 0);
            let linear = crate::prepare_raster_for_display_with_budget(&source, Some(&root)).unwrap();
            let RasterPixels::Rgba32Float(values) = linear.pixels() else {
                panic!()
            };
            // Custom D50 basis reference computed by independent Gaussian
            // RGB/XYZ inversion and Bradford adaptation (probe-linear-color-oracle.py).
            let coefficients = if custom {
                [1.5275477583865709, -0.04092907763779932, -0.010663721402315232]
            } else if p3 {
                [1.224940176, -0.042056955, -0.019637555]
            } else {
                [1.; 3]
            };
            let mut maximum_error = 0_f64;
            for (code, rgba) in values.chunks_exact(4).enumerate() {
                let value = (code as f64 / 65535.).powf(100000. / f64::from(gamma));
                for c in 0..3 {
                    maximum_error = maximum_error.max((f64::from(rgba[c]) - value * coefficients[c]).abs());
                }
                assert!((rgba[3] - 32768. / 65535.).abs() < 1e-7);
            }
            eprintln!(
                "PNG gAMA/cHRM gamma={gamma} p3={p3} custom={custom} maximum_error={maximum_error}; white={:?}",
                &values[values.len() - 4..]
            );
            assert!(maximum_error < 0.0001);
            drop(linear);
            assert_eq!(root.used(), 0);
        }
    }

    #[test]
    fn png_gray_gamma_only_preserves_full_u16_ramp_and_alpha() {
        for gamma in [50000u32, 45455, 100000] {
            for (sixteen, alpha) in [(true, true), (true, false), (false, true), (false, false)] {
                let levels = if sixteen { 65536 } else { 256 };
                let image = match (sixteen, alpha) {
                    (true, true) => {
                        DynamicImage::ImageLumaA16(image::ImageBuffer::from_fn(levels, 1, |x, _| {
                            image::LumaA([x as u16, 32768])
                        }))
                    }
                    (true, false) => {
                        DynamicImage::ImageLuma16(image::ImageBuffer::from_fn(levels, 1, |x, _| {
                            image::Luma([x as u16])
                        }))
                    }
                    (false, true) => {
                        DynamicImage::ImageLumaA8(image::ImageBuffer::from_fn(levels, 1, |x, _| {
                            image::LumaA([x as u8, 128])
                        }))
                    }
                    (false, false) => {
                        DynamicImage::ImageLuma8(image::ImageBuffer::from_fn(levels, 1, |x, _| {
                            image::Luma([x as u8])
                        }))
                    }
                };
                let mut encoded = Cursor::new(Vec::new());
                image.write_to(&mut encoded, ImageFormat::Png).unwrap();
                let base = encoded.into_inner();
                let mut bytes = base[..33].to_vec();
                bytes.extend_from_slice(&crate::png_color::tests::chunk(b"gAMA", &gamma.to_be_bytes()));
                bytes.extend_from_slice(&base[33..]);
                let source = decode_fixture("gray-gamma.png", &bytes).unwrap();
                let maximum_code = if sixteen { 65535u32 } else { 255 };
                let alpha_code = if alpha {
                    if sixteen { 32768 } else { 128 }
                } else {
                    maximum_code
                };
                match source.pixels() {
                    RasterPixels::Rgba16(native) if sixteen => {
                        for (code, rgba) in native.chunks_exact(4).enumerate() {
                            assert_eq!(rgba, &[code as u16, code as u16, code as u16, alpha_code as u16]);
                        }
                    }
                    RasterPixels::Rgba8(native) if !sixteen => {
                        for (code, rgba) in native.chunks_exact(4).enumerate() {
                            assert_eq!(rgba, &[code as u8, code as u8, code as u8, alpha_code as u8]);
                        }
                    }
                    _ => panic!("source precision changed"),
                }
                let root = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
                assert!(
                    crate::prepare_raster_for_display_with_budget_and_cancel(&source, Some(&root), || root
                        .used()
                        > 0)
                    .is_err()
                );
                assert_eq!(root.used(), 0);
                let linear = crate::prepare_raster_for_display_with_budget(&source, Some(&root)).unwrap();
                let RasterPixels::Rgba32Float(values) = linear.pixels() else {
                    panic!()
                };
                let mut maximum = 0f64;
                for (code, rgba) in values.chunks_exact(4).enumerate() {
                    let reference = (code as f64 / f64::from(maximum_code)).powf(100000. / f64::from(gamma));
                    for value in &rgba[..3] {
                        maximum = maximum.max((f64::from(*value) - reference).abs());
                    }
                    assert!((rgba[3] - alpha_code as f32 / maximum_code as f32).abs() < 1e-7);
                }
                eprintln!("gray gAMA={gamma} sixteen={sixteen} alpha={alpha} maximum_error={maximum}");
                assert!(maximum < 0.0001);
                drop(linear);
                assert_eq!(root.used(), 0);
            }
        }
    }

    #[test]
    fn png_packed_gray_gamma_and_trns_preserve_levels() {
        use crate::png_color::tests::chunk;
        use std::io::Write;
        for depth in [1u8, 2, 4] {
            for gamma in [50000u32, 45455, 100000] {
                for transparent in [false, true] {
                    let maximum = (1u8 << depth) - 1;
                    let mut row = vec![0u8; 1 + 16 * usize::from(depth) / 8];
                    for x in 0..16usize {
                        let code = x as u8 & maximum;
                        let bit = x * usize::from(depth);
                        row[1 + bit / 8] |= code << (8 - usize::from(depth) - bit % 8);
                    }
                    let mut compressor =
                        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                    compressor.write_all(&row).unwrap();
                    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
                    let mut ihdr = Vec::new();
                    ihdr.extend_from_slice(&16u32.to_be_bytes());
                    ihdr.extend_from_slice(&1u32.to_be_bytes());
                    ihdr.extend_from_slice(&[depth, 0, 0, 0, 0]);
                    bytes.extend_from_slice(&chunk(b"IHDR", &ihdr));
                    bytes.extend_from_slice(&chunk(b"gAMA", &gamma.to_be_bytes()));
                    if transparent {
                        bytes.extend_from_slice(&chunk(b"tRNS", &1u16.to_be_bytes()));
                    }
                    bytes.extend_from_slice(&chunk(b"IDAT", &compressor.finish().unwrap()));
                    bytes.extend_from_slice(&chunk(b"IEND", &[]));
                    let source = decode_fixture("packed-gray.png", &bytes).unwrap();
                    let RasterPixels::Rgba8(native) = source.pixels() else {
                        panic!("packed gray must expand to u8")
                    };
                    for (x, rgba) in native.chunks_exact(4).enumerate() {
                        let code = x as u8 & maximum;
                        let expanded = code * (255 / maximum);
                        let alpha = if transparent && code == 1 { 0 } else { 255 };
                        assert_eq!(rgba, &[expanded, expanded, expanded, alpha]);
                    }
                    let root = rrrah_core::MemoryBudget::new(4096);
                    let ready = crate::prepare_raster_for_display_with_budget(&source, Some(&root)).unwrap();
                    let RasterPixels::Rgba32Float(values) = ready.pixels() else {
                        panic!()
                    };
                    let mut error = 0f64;
                    for (x, rgba) in values.chunks_exact(4).enumerate() {
                        let code = x as u8 & maximum;
                        let reference =
                            (f64::from(code) / f64::from(maximum)).powf(100000. / f64::from(gamma));
                        for value in &rgba[..3] {
                            error = error.max((f64::from(*value) - reference).abs());
                        }
                        assert_eq!(rgba[3], if transparent && code == 1 { 0. } else { 1. });
                    }
                    assert!(
                        error < 0.000001,
                        "depth={depth} gamma={gamma} tRNS={transparent} error={error}"
                    );
                    drop(ready);
                    assert_eq!(root.used(), 0);
                }
            }
        }
    }

    #[test]
    fn png_cicp_wide_linear_hdr_and_narrow_range_reach_public_apis() {
        let image = DynamicImage::ImageRgba16(image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::from_fn(
            3,
            1,
            |x, _| {
                let value = [0, 32768, 65535][x as usize];
                image::Rgba([value, value, value, 32768])
            },
        ));
        let mut encoded = Cursor::new(Vec::new());
        image.write_to(&mut encoded, ImageFormat::Png).unwrap();
        let base = encoded.into_inner();
        for (primaries, transfer, range) in [
            (9, 8, 1),
            (12, 8, 1),
            (12, 13, 1),
            (9, 16, 1),
            (9, 18, 1),
            (1, 1, 0),
            (9, 16, 0),
            (9, 18, 0),
        ] {
            let mut bytes = base[..33].to_vec();
            bytes.extend_from_slice(&crate::png_color::tests::chunk(
                b"cICP",
                &[primaries, transfer, 0, range],
            ));
            bytes.extend_from_slice(&base[33..]);
            let frame = decode_fixture(&format!("cicp-{primaries}-{transfer}-{range}.png"), &bytes).unwrap();
            assert_eq!(
                frame.color_space(),
                &RasterColorSpace::Cicp {
                    primaries: u16::from(primaries),
                    transfer: u16::from(transfer),
                    matrix: 0,
                    full_range: range == 1,
                }
            );
            let budget = rrrah_core::MemoryBudget::new(48);
            if range == 0 {
                if transfer == 1 {
                    let linear =
                        crate::prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
                    let RasterPixels::Rgba32Float(values) = linear.pixels() else {
                        panic!()
                    };
                    assert!(values[0] < 0. && values[8] > 1.);
                    for rgba in values.chunks_exact(4) {
                        assert!((rgba[3] - 32768. / 65535.).abs() < 1e-7);
                    }
                    drop(linear);
                } else {
                    assert!(crate::prepare_raster_for_display_with_budget(&frame, Some(&budget)).is_err());
                }
                assert_eq!(budget.used(), 0);
                if transfer == 16 || transfer == 18 {
                    let path = std::env::temp_dir()
                        .join(format!("rrrah-png-narrow-{}-{transfer}.png", std::process::id()));
                    std::fs::write(&path, &bytes).unwrap();
                    let root = rrrah_core::MemoryBudget::new(1024 * 1024);
                    let mut request = DecodeRequest::new(&path);
                    request.memory_budget = Some(root.clone());
                    if transfer == 16 {
                        request.pq_reference_white_nits = Some(100.);
                    } else {
                        request.hlg_display = Some((100., 1000., 1.2));
                    }
                    let result = decode_raster(&request);
                    std::fs::remove_file(&path).unwrap();
                    assert!(result.is_err());
                    assert_eq!(root.used(), 0);
                }
                continue;
            }
            if transfer == 16 || transfer == 18 {
                assert!(crate::prepare_raster_for_display_with_budget(&frame, Some(&budget)).is_err());
                assert_eq!(budget.used(), 0);
            }
            let linear = match transfer {
                16 => frame
                    .pq_to_linear_srgb_with_budget_and_cancel(100., Some(&budget), || false)
                    .unwrap(),
                18 => frame
                    .hlg_to_linear_srgb_with_budget_and_cancel(100., 1000., 1.2, Some(&budget), || false)
                    .unwrap(),
                _ => crate::prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap(),
            };
            let RasterPixels::Rgba32Float(values) = linear.pixels() else {
                panic!()
            };
            let white = match transfer {
                16 => 100.,
                18 => 10.,
                _ => 1.,
            };
            for channel in 0..3 {
                assert_eq!(values[channel], 0.);
                assert!((values[8 + channel] - white).abs() < 0.0001);
                assert!(values[4 + channel] > 0. && values[4 + channel] < white);
            }
            for rgba in values.chunks_exact(4) {
                assert!((rgba[3] - 32768. / 65535.).abs() < 1e-7);
            }
            assert_eq!(budget.used(), 48);
            if transfer == 16 || transfer == 18 {
                let path = std::env::temp_dir()
                    .join(format!("rrrah-png-policy-{}-{transfer}.png", std::process::id()));
                std::fs::write(&path, &bytes).unwrap();
                let root = rrrah_core::MemoryBudget::new(1024 * 1024);
                let mut request = DecodeRequest::new(&path);
                request.memory_budget = Some(root.clone());
                if transfer == 16 {
                    request.pq_reference_white_nits = Some(100.);
                } else {
                    request.hlg_display = Some((100., 1000., 1.2));
                }
                let result = decode_raster(&request);
                std::fs::remove_file(&path).unwrap();
                let policy_frame = result.unwrap();
                assert_eq!(policy_frame.color_space(), &RasterColorSpace::LinearSrgb);
                let RasterPixels::Rgba32Float(actual) = policy_frame.pixels() else {
                    panic!()
                };
                assert_eq!(actual.as_slice(), values.as_slice());
                drop(policy_frame);
                assert_eq!(root.used(), 0);
            }
            drop(linear);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn qoi_transfer_flag_changes_color_interpretation_without_changing_alpha() {
        let fixture = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([128, 128, 128, 64]),
        ));
        let mut encoded = Cursor::new(Vec::new());
        fixture.write_to(&mut encoded, ImageFormat::Qoi).unwrap();
        let mut bytes = encoded.into_inner();
        for flag in [0_u8, 1] {
            bytes[13] = flag;
            let frame = decode_fixture(&format!("transfer-{flag}.qoi"), &bytes).unwrap();
            let linear = frame.to_linear_srgb().unwrap();
            let RasterPixels::Rgba32Float(p) = linear.pixels() else {
                panic!("wrong precision")
            };
            let expected = if flag == 0 { 0.2158605 } else { 128.0 / 255.0 };
            assert!((p[0] - expected).abs() < 1e-6);
            assert!((p[3] - 64.0 / 255.0).abs() < 1e-7);
        }
    }

    fn decode_fixture(name: &str, bytes: &[u8]) -> Result<DecodedRaster, RasterDecodeError> {
        let path = std::env::temp_dir().join(format!("rrrah-raster-{}-{name}", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        let result = decode_raster_file(&path);
        std::fs::remove_file(path).unwrap();
        result
    }

    #[test]
    fn applies_exif_rotation_and_retains_icc_bytes() {
        // Little-endian TIFF EXIF, one SHORT Orientation tag = 6 (90 CW).
        let exif = vec![
            b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
        ];
        let profile = vec![0_u8; 128];
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        encoder.set_exif_metadata(exif).unwrap();
        encoder.set_icc_profile(profile.clone()).unwrap();
        encoder
            .write_image(
                &[255, 0, 0, 255, 0, 255, 0, 127],
                2,
                1,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        let frame = decode_fixture("oriented.png", &bytes).unwrap();
        assert_eq!((frame.width(), frame.height()), (1, 2));
        assert_eq!(frame.color_space(), &RasterColorSpace::Icc(profile));
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!("unexpected precision")
        };
        assert_eq!(p.as_slice(), [255, 0, 0, 255, 0, 255, 0, 127]);
    }

    #[test]
    fn radiance_hdr_values_are_not_clipped_to_one() {
        let mut bytes = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 2\n".to_vec();
        bytes.extend_from_slice(&[128, 64, 32, 131, 64, 32, 16, 130]);
        let frame = decode_fixture("hdr.jpg", &bytes).unwrap();
        let RasterPixels::Rgba32Float(p) = frame.pixels() else {
            panic!("HDR precision lost")
        };
        assert_eq!(p.as_slice(), [4.0, 2.0, 1.0, 1.0, 1.0, 0.5, 0.25, 1.0]);
        assert_eq!(frame.color_space(), &RasterColorSpace::LinearRgbUnspecified);
    }

    #[test]
    fn malformed_and_oversized_sources_fail_without_output_allocation() {
        assert!(decode_fixture("truncated.png", b"\x89PNG\r\n\x1a\n").is_err());
        assert!(decode_fixture("huge.ppm", b"P6\n65536 65536\n255\n").is_err());
        assert!(matches!(
            decode_fixture("garbage.jpg", b"not an image"),
            Err(RasterDecodeError::UnsupportedFormat)
        ));
    }

    #[test]
    fn pfm_magic_routes_through_public_api_despite_jpeg_extension() {
        let mut bytes = b"Pf\n1 1\n-2\n".to_vec();
        bytes.extend_from_slice(&3.5_f32.to_le_bytes());
        let frame = decode_fixture("pfm-content.jpg", &bytes).unwrap();
        assert_eq!(frame.sample_scale(), 2.0);
        let RasterPixels::Rgba32Float(p) = frame.pixels() else {
            panic!("PFM precision lost")
        };
        assert_eq!(p.as_slice(), [3.5, 3.5, 3.5, 1.0]);
    }

    #[test]
    fn lossless_containers_preserve_pixels_and_alpha_despite_wrong_extension() {
        let source = image::RgbaImage::from_raw(2, 1, vec![17, 29, 43, 255, 61, 73, 89, 127]).unwrap();
        for format in [
            ImageFormat::Png,
            ImageFormat::Tiff,
            ImageFormat::Qoi,
            ImageFormat::Farbfeld,
        ] {
            let mut encoded = Cursor::new(Vec::new());
            let mut fixture = DynamicImage::ImageRgba8(source.clone());
            if format == ImageFormat::Farbfeld {
                fixture = DynamicImage::ImageRgba16(fixture.into_rgba16());
            }
            fixture.write_to(&mut encoded, format).unwrap();
            let path =
                std::env::temp_dir().join(format!("rrrah-raster-{}-{format:?}.jpg", std::process::id()));
            std::fs::write(&path, encoded.into_inner()).unwrap();
            let result = decode_raster_file(&path);
            std::fs::remove_file(path).unwrap();
            let frame = result.unwrap();
            assert_eq!((frame.width(), frame.height()), (2, 1));
            match frame.pixels() {
                RasterPixels::Rgba8(p) => assert_eq!(p.as_slice(), source.as_raw()),
                RasterPixels::Rgba16(p) => assert_eq!(
                    p.as_slice(),
                    source
                        .as_raw()
                        .iter()
                        .map(|v| u16::from(*v) * 257)
                        .collect::<Vec<_>>()
                ),
                RasterPixels::Rgba32Float(_) => panic!("unexpected precision"),
            }
        }
    }

    #[test]
    fn png_retains_all_sixteen_bits() {
        let samples = vec![1_u16, 257, 32769, 65534];
        let fixture =
            image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::from_raw(1, 1, samples.clone()).unwrap();
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba16(fixture)
            .write_to(&mut encoded, ImageFormat::Png)
            .unwrap();
        let path = std::env::temp_dir().join(format!("rrrah-raster-{}-16.png", std::process::id()));
        std::fs::write(&path, encoded.into_inner()).unwrap();
        let frame = decode_raster_file(&path).unwrap();
        let bytes = std::fs::metadata(&path).unwrap().len();
        let budget = rrrah_core::MemoryBudget::new(bytes + frame.pixel_capacity_bytes());
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let managed = decode_raster(&request).unwrap();
        let (RasterPixels::Rgba16(a), RasterPixels::Rgba16(b)) = (managed.pixels(), frame.pixels()) else {
            panic!("16-bit precision lost");
        };
        assert_eq!(a, b);
        assert_eq!(budget.peak(), bytes + managed.pixel_capacity_bytes());
        assert!(a.is_managed());
        assert_eq!(budget.used(), managed.pixel_capacity_bytes());
        drop(managed);
        assert_eq!(budget.used(), 0);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(0));
        assert!(matches!(
            decode_raster(&request),
            Err(RasterDecodeError::Source(DecodeError::Memory(_)))
        ));
        std::fs::remove_file(path).unwrap();
        let RasterPixels::Rgba16(p) = frame.pixels() else {
            panic!("16-bit precision lost")
        };
        assert_eq!(p.as_slice(), samples);
    }

    #[test]
    fn rejects_stale_requests_and_unselectable_image_indices() {
        let mut request = DecodeRequest::new("missing.png");
        request.image_index = 1;
        assert!(matches!(
            decode_raster(&request),
            Err(RasterDecodeError::Source(DecodeError::UnsupportedImageIndex {
                index: 1
            }))
        ));
        request.image_index = 0;
        request.cancellation = Some(crate::GenerationToken::new(
            Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            decode_raster(&request),
            Err(RasterDecodeError::Source(DecodeError::Cancelled))
        ));
    }
}

#[cfg(test)]
mod output_budget_tests {
    use super::*;
    #[test]
    fn output_admission_precedes_png_entropy_decode_and_errors_release_reservation() {
        let image = image::RgbaImage::from_pixel(64, 64, image::Rgba([17, 29, 41, 255]));
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut encoded, ImageFormat::Png)
            .unwrap();
        let mut bytes = encoded.into_inner();
        let idat = bytes.windows(4).position(|w| w == b"IDAT").unwrap();
        bytes[idat + 4] ^= 0xff; // broken zlib stream, intact dimensions
        let mut request = DecodeRequest::new("broken-entropy.png");
        assert!(matches!(
            decode_raster_bytes(bytes.clone(), &request),
            Err(RasterDecodeError::Codec(_))
        ));
        let denied = rrrah_core::MemoryBudget::new(0);
        request.memory_budget = Some(denied.clone());
        assert!(matches!(
            decode_raster_bytes(bytes.clone(), &request),
            Err(RasterDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(denied.used(), 0);
        let admitted = rrrah_core::MemoryBudget::new(64 * 64 * 4);
        request.memory_budget = Some(admitted.clone());
        assert!(matches!(
            decode_raster_bytes(bytes, &request),
            Err(RasterDecodeError::Codec(_))
        ));
        assert_eq!(admitted.peak(), 64 * 64 * 4);
        assert_eq!(admitted.used(), 0);
    }

    #[test]
    fn raster_output_pressure_releases_input_and_retained_owners_hold_budget() {
        let image = image::RgbaImage::from_pixel(64, 64, image::Rgba([17, 29, 41, 255]));
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut encoded, ImageFormat::Png)
            .unwrap();
        let bytes = encoded.into_inner();
        let mut request = DecodeRequest::new("output-budget.png");
        let tight = rrrah_core::MemoryBudget::new(bytes.len() as u64);
        assert!(bytes.len() < 64 * 64 * 4);
        request.memory_budget = Some(tight.clone());
        assert!(matches!(
            decode_raster_bytes(bytes.clone(), &request),
            Err(RasterDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(tight.used(), 0);
        let budget = rrrah_core::MemoryBudget::new(64 * 64 * 4);
        request.memory_budget = Some(budget.clone());
        let raster = decode_raster_bytes(bytes, &request).unwrap();
        assert_eq!((raster.width(), raster.height()), (64, 64));
        let RasterPixels::Rgba8(values) = raster.pixels() else {
            panic!("precision changed")
        };
        assert!(values.is_managed());
        assert_eq!(&values[..4], &[17, 29, 41, 255]);
        let retained = values.clone();
        drop(raster);
        assert_eq!(budget.used(), 64 * 64 * 4);
        drop(retained);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn big_tiff_header_maps_to_tiff_decoder() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../rrrah-dedup/tests/fixtures/tiff");
        for name in ["bigtiff-little-none-same.tif", "bigtiff-big-lzw-same.tif"] {
            let raster = decode_raster_file(root.join(name)).unwrap();
            assert_eq!((raster.width(), raster.height()), (5, 4));
        }
    }
    #[test]
    fn standard_pnm_variants_prepare_bt709_without_srgb_assumption() {
        let cases: Vec<(&str, Vec<u8>, [[f64; 3]; 2])> = vec![
            ("p1.pbm", b"P1\n2 1\n0 1\n".to_vec(), [[1.; 3], [0.; 3]]),
            ("p4.pbm", b"P4\n2 1\n\x40".to_vec(), [[1.; 3], [0.; 3]]),
            (
                "p2.pgm",
                b"P2\n2 1\n255\n128 255\n".to_vec(),
                [[128. / 255.; 3], [1.; 3]],
            ),
            (
                "p5.pgm",
                b"P5\n2 1\n65535\n\x80\0\xff\xff".to_vec(),
                [[32768. / 65535.; 3], [1.; 3]],
            ),
            (
                "p3.ppm",
                b"P3\n2 1\n255\n255 0 0 128 128 128\n".to_vec(),
                [[1., 0., 0.], [128. / 255.; 3]],
            ),
            (
                "p6.ppm",
                b"P6\n2 1\n65535\n\xff\xff\0\0\0\0\x80\0\x80\0\x80\0".to_vec(),
                [[1., 0., 0.], [32768. / 65535.; 3]],
            ),
        ];
        for (name, bytes, expected) in cases {
            let path = std::env::temp_dir().join(format!("rrrah-pnm-bt709-{}-{name}", std::process::id()));
            std::fs::write(&path, bytes).unwrap();
            let budget = rrrah_core::MemoryBudget::new(4096);
            let mut request = crate::DecodeRequest::new(&path);
            request.memory_budget = Some(budget.clone());
            let source = decode_raster(&request).unwrap();
            std::fs::remove_file(path).unwrap();
            assert_eq!(source.color_space(), &rrrah_core::RasterColorSpace::Bt709);
            let linear = crate::prepare_raster_for_display_with_budget(&source, Some(&budget)).unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(pixels) = linear.pixels() else {
                panic!("float output");
            };
            for (actual, rgb) in pixels.chunks_exact(4).zip(expected) {
                for (actual, value) in actual[..3].iter().zip(rgb) {
                    let reference = if value < 0.081 {
                        value / 4.5
                    } else {
                        ((value + 0.099) / 1.099).powf(1. / 0.45)
                    };
                    assert!((f64::from(*actual) - reference).abs() < 3e-7, "{name}");
                }
                assert_eq!(actual[3], 1.);
            }
            drop(linear);
            drop(source);
            assert_eq!(budget.used(), 0);
        }
    }
}

#[cfg(test)]
mod native_farbfeld_tests {
    use super::*;
    #[test]
    fn native_framing_precision_and_preallocation_budget_are_checked() {
        let mut bytes = b"farbfeld".to_vec();
        bytes.extend(2u32.to_be_bytes());
        bytes.extend(1u32.to_be_bytes());
        let samples = [0u16, 1, 32768, 65535, 65534, 12345, 54321, 2];
        bytes.extend(samples.iter().flat_map(|v| v.to_be_bytes()));
        let budget = rrrah_core::MemoryBudget::new(16);
        let mut request = DecodeRequest::new("renamed.png");
        request.memory_budget = Some(budget.clone());
        let frame = decode_raster_bytes(bytes.clone(), &request).unwrap();
        let RasterPixels::Rgba16(values) = frame.pixels() else {
            panic!()
        };
        assert_eq!(&values[..], &samples);
        assert_eq!(frame.color_space(), &RasterColorSpace::AssumedSrgb);
        assert_eq!(budget.used(), 16);
        drop(frame);
        assert_eq!(budget.used(), 0);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(15));
        assert!(decode_raster_bytes(bytes.clone(), &request).is_err());
        assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
        request.memory_budget = Some(budget.clone());
        for invalid in [
            bytes[..15].to_vec(),
            bytes[..31].to_vec(),
            [bytes.as_slice(), &[0]].concat(),
        ] {
            assert!(decode_raster_bytes(invalid, &request).is_err());
            assert_eq!(budget.used(), 0);
        }
        request.image_index = 1;
        assert!(decode_raster_bytes(bytes, &request).is_err());
        assert_eq!(budget.used(), 0);
    }
}
