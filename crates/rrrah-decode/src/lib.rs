//! Unified decoding façade re-exporting `rrrah-raw`, `rrrah-mesh`, `rrrah-vector`,
//! and raster/scientific/texture decoders.
#![allow(clippy::missing_errors_doc, clippy::cast_precision_loss)]

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
mod xcf;
pub use xcf::XcfCompositionAttributes;
pub use xcf::XcfLayerAttributes;
pub use xcf::XcfLayerGroup;
pub use xcf::composite_xcf_normal_premultiplied_rgba32f;
pub use xcf::composite_xcf_normal_rgba8;
pub use xcf::composite_xcf_normal_rgba32f;
pub use xcf::decode_xcf_samples;
pub use xcf::decode_xcf_tile;
pub use xcf::expand_xcf_legacy_u8_pixels;
pub use xcf::prepare_xcf_legacy_rgba8;
pub use xcf::xcf_icc_profile;
pub use xcf::xcf_palette;
pub use xcf::{XcfChannel, parse_xcf_channel};
pub use xcf::{XcfDecodedLayer, decode_xcf_layer};
pub use xcf::{XcfFlattenedImage, flatten_xcf_legacy_normal};
pub use xcf::{XcfHeader, XcfHeaderError, XcfProperties, XcfProperty, parse_xcf_header, xcf_properties};
pub use xcf::{XcfLayer, parse_xcf_layer};
pub use xcf::{XcfLayerNode, parse_xcf_layer_tree};
pub use xcf::{XcfLevel, parse_xcf_hierarchy};
pub use xcf::{XcfObjectTables, XcfOffsets, parse_xcf_object_tables};
pub use xcf::{XcfPixelError, decode_xcf_level};

mod x3f;
pub use x3f::{
    X3fCamf, X3fCamfEntry, X3fCamfMatrix, X3fColorTransform, X3fEntry, X3fError, X3fImageInfo, X3fInventory,
    X3fLegacyChannels, X3fLegacyHuffman, X3fNoiseCurve, X3fProperties, crop_x3f_channels, decode_x3f_legacy,
    decode_x3f_legacy_auto, inspect_x3f, linearize_x3f_highlights, linearize_x3f_highlights_from_camf,
    repair_x3f_bad_pixels, repair_x3f_bad_pixels_from_camf, rotate_x3f_linear, sharpen_x3f_red,
    smooth_x3f_black_rows, smooth_x3f_chroma, smooth_x3f_hues, smooth_x3f_hues_wide, transform_x3f_pixels,
    visit_x3f_camf, x3f_block_enabled, x3f_chroma_guide, x3f_color_transform_auto, x3f_illuminant_matrix,
    x3f_linear_output, x3f_linear_raster, x3f_neutral_response, x3f_noise_curves_auto, x3f_wb_correction,
};

mod animation;
mod aseprite;
pub use aseprite::{AsepriteImage, AsepriteTag, decode_aseprite};
mod jpegls;
mod jpegxr;
mod sample;
pub use jpegls::{JpegLsImage, decode_jpegls};
mod jng;
mod webp_animation;
pub use webp_animation::{WebpImage, decode_webp_animation};
mod gif_animation;
pub use gif_animation::{GifImage, decode_gif};
mod basis;
mod mng;
mod sti;
pub use mng::{MngImage, decode_mng};
mod apng;
mod astc;
pub use apng::{ApngImage, decode_apng};
mod avif_color;
mod cineon;
mod cursor;
mod dcx;
mod dds;
mod dicom;
mod dpx;
mod exr_color;
mod fits;
mod hdr_color;
mod heif;
mod iff;
mod image_router;
mod jp2;
mod jxl;
mod kra;
mod ktx;
mod ktx2;
mod macpaint;
mod mrc;
mod nrrd;
mod ora;
mod pcx;
mod pfm;
mod pix;
mod pkm;
mod psd;
mod pvr;
mod qoi_bounds;
mod raster;
mod raster_color;
#[cfg(test)]
mod raster_corpus;
mod rla;
mod rpf;
mod scientific;
mod sgi;
mod softimage;
mod sun;
mod wad;
mod wal;
mod xbm;
mod xpm;

pub use fits::{FitsImage, FitsSamples, decode_fits};
pub use image_router::{
    DecodedImage, ImageSourceKind, decode_image, decode_image_file, image_source_kind,
    is_supported_image_path, is_supported_raw_path,
};
pub use mrc::{MrcImage, decode_mrc};
pub use nrrd::{NrrdImage, decode_nrrd};
pub use pvr::{PvrImage, PvrMetadata, decode_pvr};
pub use raster::{RasterDecodeError, decode_raster, decode_raster_file};
pub use raster_color::{
    RasterColorError, prepare_raster_for_display, prepare_raster_for_display_with_budget,
    prepare_raster_for_display_with_budget_and_cancel,
};
pub use rla::{decode_rla_float_with_interpretation, decode_rla_with_interpretation};
pub use rpf::{
    RPF_GBUFFER_SAMPLE_BYTES, RpfAspectError, RpfDecodeLimits, RpfDecodedImage, RpfDecodedLayers,
    RpfDisplayError, RpfFileError, RpfFileView, RpfHeader, RpfImageAllocationPlan, RpfImageDecodeError,
    RpfImageSnapshot, RpfInspectError, RpfInspection, RpfLayerCoordinateError, RpfLayerDecodeError,
    RpfLayerError, RpfLayerRecords, RpfNameError, RpfNodeNames, RpfPlanError, RpfPlaneError,
    RpfRasterReadError, RpfReadError, RpfRenderInfo, RpfRenderInfoError, RpfRowError, RpfRowView,
    RpfSnapshotError, RpfWindow, decode_rpf_byte_planes, decode_rpf_color_channel,
    decode_rpf_file_raster_with_budget, decode_rpf_file_with_budget, inspect_rpf, inspect_rpf_file,
    inspect_rpf_layer_records, inspect_rpf_node_names, inspect_rpf_render_info, inspect_rpf_row,
};
pub use scientific::{ScalarWindow, decode_raster_with_window};
pub use wal::{decode_wal_with_palette, wal_palette_path};

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
