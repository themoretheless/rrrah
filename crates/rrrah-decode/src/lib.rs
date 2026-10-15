//! Unified decoding façade re-exporting `rrrah-raw`, `rrrah-mesh`, `rrrah-vector`,
//! and raster/scientific/texture decoders.
#![allow(
    clippy::pedantic,
    clippy::chunks_exact_to_as_chunks,
    clippy::collapsible_if,
    clippy::missing_errors_doc,
    clippy::cast_precision_loss,
    clippy::manual_is_multiple_of,
    clippy::manual_checked_ops,
    clippy::too_many_arguments,
    clippy::manual_div_ceil,
    clippy::needless_range_loop,
    clippy::manual_range_patterns,
    clippy::precedence,
    clippy::needless_borrow
)]

use std::path::PathBuf;

pub use rrrah_raw::{
    AdaptTimings, BayReadError, BaySensorError, BaySensorLayout, DecodeError, DecodeOutput, DecodeRequest,
    DecodeTimings, DngDecodeTimings, EipEntry, EipError, EipManifest, EipRaw, EipSensor, EpsonMakerMetadata,
    GenerationToken, MrwLayout, NATIVE_CR3_BACKEND_ID, NATIVE_DNG_BACKEND_ID, NATIVE_DNG_MOSAIC_CONTRACT_1,
    NATIVE_EOS_R8_MOSAIC_CONTRACT_1, NativeCr3Decoder, NativeCrwDecoder, NativeDecodeTimings,
    NativeDngDecoder, NativeMrwDecoder, NativeRawDecoder, OPTIO_S4_SENSOR_SIZE, OptioS4SensorError,
    RawDecoder, RlaAlphaMode, RlaColorSpace, RlaFloatByteOrder, RpfRasterInterpretation, WORKSPACE_LOCK_DIGEST,
    bounded_io, decode_crw_10d, decode_crw_10d_with_tables, decode_eip_file_sensor, decode_eip_sensor,
    decode_file, decode_mef_zd_sensor, decode_mef_zd_sensor_file, dng, eip_file_sensor_recipe, inspect_eip,
    raw_development_opcodes, read_bay_image_sensor_with_budget, read_bay_sensor_with_budget, read_eip_raw,
    read_eip_raw_with_budget, read_erf_maker_metadata, read_optio_s4_sensor_with_budget, sniff,
    unpack_bay_sensor, unpack_optio_s4_sensor, vc5,
};

pub use rrrah_mesh::{
    DecodedModel, ModelBuffer, ModelDecodeError, ObjCorner, ObjDecodeError, ObjFace, ObjMesh, ObjSwapError,
    ObjTriangle, OffColor, OffDecodeError, OffEncoding, OffFace, OffMesh, OffSwapError, PlyDecodeError,
    PlyElement, PlyEncoding, PlyMesh, PlyProperty, PlyScalars, PlySwapError, PlyType, PlyValues,
    StlDecodeError, StlFacet, StlMesh, StlSwapError, decode_model, decode_obj, decode_off, decode_ply,
    decode_stl, is_supported_model_path, obj_swap_payload_len, off_swap_payload_len, ply_swap_payload_len,
    read_obj_swap_payload, read_off_swap_payload, read_ply_swap_payload, read_stl_swap_payload,
    stl_swap_payload_len, write_obj_swap_payload, write_off_swap_payload, write_ply_swap_payload,
    write_stl_swap_payload,
};

pub use rrrah_vector::{
    EpsBounds, EpsBoundsError, EpsCompileError, EpsCompileLimits, EpsDeviceColor, EpsDocumentError,
    EpsDocumentLimits, EpsEvaluation, EpsFillError, EpsFillLimits, EpsFlattenError, EpsFlattenLimits,
    EpsGraphics, EpsGraphicsError, EpsGraphicsLimit, EpsGraphicsLimits, EpsInspectError, EpsInstruction,
    EpsMatrix, EpsNumber, EpsNumberError, EpsOperator, EpsPaint, EpsPaintKind, EpsPathSegment, EpsPoint,
    EpsPreparedStroke, EpsProgram, EpsRasterColorPolicy, EpsRasterError, EpsRasterLimits, EpsSource,
    EpsStringError, EpsStrokeAssemblyLimits, EpsStrokeError, EpsStrokePrepareError, EpsStrokeStyle, EpsToken,
    EpsTokenError, EpsTokens, EpsValue, EpsVectorExecution, EpsVectorScene, EpsVmError, EpsVmLimit,
    EpsVmLimits, compile_eps_program, decode_eps_document, decode_eps_file_document, decode_eps_string, emf,
    eps, eps_bounds, eps_document, eps_fill, eps_flatten, eps_graphics, eps_numbers, eps_program, eps_raster,
    eps_strings, eps_stroke, eps_stroke_prepare, eps_tokens, eps_vm, evaluate_eps_program,
    evaluate_eps_vectors, fill_eps_path, flatten_eps_path, inspect_eps_bounds, inspect_eps_source,
    outline_eps_join, outline_eps_polyline, outline_eps_prepared_stroke, outline_eps_segment,
    parse_eps_number, pdf, pict, png_color, prepare_eps_stroke, rasterize_eps_fills, rasterize_eps_scene, svg,
    wmf,
};

mod jpeg_cmyk;

pub use rrrah_layered::{
    AsepriteImage, AsepriteTag, LayeredDecodeError, XcfChannel, XcfCompositionAttributes, XcfDecodedLayer,
    XcfFlattenedImage, XcfHeader, XcfHeaderError, XcfLayer, XcfLayerAttributes, XcfLayerGroup, XcfLayerNode,
    XcfLevel, XcfObjectTables, XcfOffsets, XcfPixelError, XcfProperties, XcfProperty, aseprite,
    composite_xcf_normal_premultiplied_rgba32f, composite_xcf_normal_rgba8, composite_xcf_normal_rgba32f,
    decode_xcf_layer, decode_xcf_level, decode_xcf_samples, decode_xcf_tile, expand_xcf_legacy_u8_pixels,
    flatten_xcf_legacy_normal, parse_xcf_channel, parse_xcf_header, parse_xcf_hierarchy, parse_xcf_layer,
    parse_xcf_layer_tree, parse_xcf_object_tables, prepare_xcf_legacy_rgba8, psd, xcf, xcf_icc_profile,
    xcf_palette, xcf_properties,
};

pub fn decode_aseprite(request: &DecodeRequest) -> Result<AsepriteImage, RasterDecodeError> {
    Ok(rrrah_layered::decode_aseprite(request)?)
}

pub use rrrah_raw::{
    X3fCamf, X3fCamfEntry, X3fCamfMatrix, X3fColorTransform, X3fEntry, X3fError, X3fImageInfo, X3fInventory,
    X3fLegacyChannels, X3fLegacyHuffman, X3fNoiseCurve, X3fProperties, crop_x3f_channels, decode_x3f_legacy,
    decode_x3f_legacy_auto, inspect_x3f, linearize_x3f_highlights, linearize_x3f_highlights_from_camf,
    repair_x3f_bad_pixels, repair_x3f_bad_pixels_from_camf, rotate_x3f_linear, sharpen_x3f_red,
    smooth_x3f_black_rows, smooth_x3f_chroma, smooth_x3f_hues, smooth_x3f_hues_wide, transform_x3f_pixels,
    visit_x3f_camf, x3f, x3f_block_enabled, x3f_chroma_guide, x3f_color_transform_auto,
    x3f_illuminant_matrix, x3f_linear_output, x3f_linear_raster, x3f_neutral_response, x3f_noise_curves_auto,
    x3f_wb_correction,
};

pub use rrrah_scientific::{
    FitsImage, FitsSamples, MrcImage, NrrdImage, ScalarWindow, ScientificDecodeError, dicom, fits, mrc,
    nrrd, sample, scientific,
};

pub fn decode_fits(request: &DecodeRequest) -> Result<FitsImage, RasterDecodeError> {
    Ok(rrrah_scientific::decode_fits(request)?)
}

pub fn decode_mrc(request: &DecodeRequest) -> Result<MrcImage, RasterDecodeError> {
    Ok(rrrah_scientific::decode_mrc(request)?)
}

pub fn decode_nrrd(request: &DecodeRequest) -> Result<NrrdImage, RasterDecodeError> {
    Ok(rrrah_scientific::decode_nrrd(request)?)
}

pub fn decode_raster_with_window(
    request: &DecodeRequest,
    window: ScalarWindow,
) -> Result<rrrah_core::DecodedRaster, RasterDecodeError> {
    Ok(rrrah_scientific::decode_raster_with_window(request, window)?)
}

pub use rrrah_vfx::{
    RPF_GBUFFER_SAMPLE_BYTES, RpfAspectError, RpfDecodeLimits, RpfDecodedImage, RpfDecodedLayers,
    RpfDisplayError, RpfFileError, RpfFileView, RpfHeader, RpfImageAllocationPlan, RpfImageDecodeError,
    RpfImageSnapshot, RpfInspectError, RpfInspection, RpfLayerCoordinateError, RpfLayerDecodeError,
    RpfLayerError, RpfLayerRecords, RpfNameError, RpfNodeNames, RpfPlanError, RpfPlaneError,
    RpfRasterReadError, RpfReadError, RpfRenderInfo, RpfRenderInfoError, RpfRowError, RpfRowView,
    RpfSnapshotError, RpfWindow, VfxDecodeError, cineon, decode_rpf_byte_planes, decode_rpf_color_channel,
    decode_rpf_file_raster_with_budget, decode_rpf_file_with_budget, dpx, inspect_rpf, inspect_rpf_file,
    inspect_rpf_layer_records, inspect_rpf_node_names, inspect_rpf_render_info, inspect_rpf_row, pix, rla,
    rpf, sgi, softimage,
};

pub fn decode_rla_float_with_interpretation(
    request: &DecodeRequest,
    byte_order: RlaFloatByteOrder,
    alpha: RlaAlphaMode,
    color: rrrah_core::RasterColorSpace,
) -> Result<rrrah_core::DecodedRaster, RasterDecodeError> {
    Ok(rrrah_vfx::decode_rla_float_with_interpretation(
        request, byte_order, alpha, color,
    )?)
}

pub fn decode_rla_with_interpretation(
    request: &DecodeRequest,
    alpha: RlaAlphaMode,
    color: rrrah_core::RasterColorSpace,
) -> Result<rrrah_core::DecodedRaster, RasterDecodeError> {
    Ok(rrrah_vfx::decode_rla_with_interpretation(request, alpha, color)?)
}

pub use rrrah_texture::{
    PvrImage, PvrMetadata, TextureDecodeError, astc, basis, dcx, dds, ktx, ktx2, pcx, pkm, pvr, sti, wad,
    wal, wal_palette_path,
};

pub fn decode_pvr(request: &DecodeRequest) -> Result<PvrImage, RasterDecodeError> {
    Ok(rrrah_texture::decode_pvr(request)?)
}

pub fn decode_wal_with_palette(
    request: &DecodeRequest,
    palette: &[[u8; 4]; 256],
    color_space: rrrah_core::RasterColorSpace,
) -> Result<rrrah_core::DecodedRaster, RasterDecodeError> {
    Ok(rrrah_texture::decode_wal_with_palette(
        request,
        palette,
        color_space,
    )?)
}

pub use rrrah_animation::{
    AnimationDecodeError, ApngImage, GifImage, RasterColorError, WebpImage, animation, apng, gif_animation,
    jng, prepare_raster_for_display, prepare_raster_for_display_with_budget,
    prepare_raster_for_display_with_budget_and_cancel, raster_color, webp_animation,
};

pub fn decode_apng(request: &DecodeRequest) -> Result<ApngImage, RasterDecodeError> {
    Ok(rrrah_animation::decode_apng(request)?)
}

pub fn decode_gif(request: &DecodeRequest) -> Result<GifImage, RasterDecodeError> {
    Ok(rrrah_animation::decode_gif(request)?)
}

pub fn decode_webp_animation(request: &DecodeRequest) -> Result<WebpImage, RasterDecodeError> {
    Ok(rrrah_animation::decode_webp_animation(request)?)
}

mod jpegls;
mod jpegxr;
pub use jpegls::{JpegLsImage, decode_jpegls};
mod mng;
pub use mng::{MngImage, decode_mng};
mod avif_color;
mod cursor;
mod exr_color;
mod hdr_color;
mod heif;
mod iff;
mod image_router;
mod jp2;
mod jxl;
mod kra;
mod macpaint;
mod ora;
mod pfm;
mod qoi_bounds;
mod raster;
#[cfg(test)]
mod raster_corpus;
mod sun;
mod xbm;
mod xpm;

pub use image_router::{
    DecodedImage, ImageSourceKind, decode_image, decode_image_file, image_source_kind,
    is_supported_image_path, is_supported_raw_path,
};
pub use raster::{RasterDecodeError, decode_raster, decode_raster_file};

/// Identity of the immutable system-font snapshot used by SVG text rendering.
///
/// # Errors
/// Returns an error if a discovered font cannot be retained.
pub fn svg_font_resource_identity() -> Result<[u8; 32], RasterDecodeError> {
    Ok(svg::font_identity()?)
}

/// Identity of the currently available HEVC decoder inventory, ordered by priority.
/// This is capability/version identity, not a digest of native library bytes.
///
/// # Errors
/// Returns initialization errors or an excessive decoder inventory error.
pub fn hevc_decoder_resource_identity() -> Result<[u8; 32], RasterDecodeError> {
    heif::decoder_identity()
}

/// External files consumed by native raster dispatch for this request/header.
/// Extension hints for otherwise recognized raster data do not add WAL dependencies.
pub fn raster_external_dependencies(request: &DecodeRequest, header: &[u8]) -> Vec<PathBuf> {
    if image::guess_format(header).is_err() {
        wal::wal_palette_path(&request.path).into_iter().collect()
    } else {
        Vec::new()
    }
}
