//! Explicit model-input projections. These model pixels never replace
//! original normalized pixels in duplicate confirmation.
use crate::{linear::LinearRgbaView, model_projection::ModelInputProjection};
use rrrah_core::{MemoryBudget, SharedBuffer};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelInputError {
    #[error("invalid model-input dimensions or policy")]
    Invalid,
    #[error("model-input allocation or work admission exceeded")]
    Budget,
    #[error("model-input preparation cancelled")]
    Cancelled,
    #[error("pixels are unsupported by the selected model-input projection")]
    UnsupportedPixels,
}
#[derive(Debug, Clone, Copy)]
pub struct ModelInputPolicy {
    pub max_side: u32,
    pub max_source_pixels: u64,
    pub max_output_pixels: u64,
    pub max_pixel_reads: u64,
}
#[derive(Debug, Clone)]
pub struct GrayModelInput {
    pub dimensions: [u32; 2],
    pub original: [u32; 2],
    pub pixels: SharedBuffer<u8>,
}

/// Area-average encoded sRGB BT.601 luminance onto dimensions divisible by eight.
/// The largest side is bounded by `max_side`; tiny sources may be upsampled to
/// eight pixels for network compatibility. Integer overlap weights include every
/// source edge, including odd dimensions. Output is rounded once to grayscale8.
/// Every source read observes cancellation and a work cap. Output payload is
/// admitted before reads and retained by its last shared owner. No full-size
/// grayscale scratch is allocated. Source storage/allocator overhead is excluded.
/// Alpha/HDR refusal is explicit; this recipe neither composites nor tone maps.
///
/// # Errors
/// Invalid policy, unsupported pixels, allocation/count/work excess or cancellation.
pub fn prepare_gray_model_input(
    image: &LinearRgbaView<'_>,
    policy: ModelInputPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<GrayModelInput, ModelInputError> {
    let (width, height) = image.dimensions();
    prepare_gray_model_region_input(image, [0, 0, width, height], policy, budget, &cancel)
}

/// Prepare a fixed original-pixel rectangle without copying a full crop raster.
/// `GrayModelInput::original` describes the crop extent, not the full image;
/// callers must retain and apply the rectangle origin when mapping proposals.
/// The full original image still obeys `max_source_pixels`. Pixel reads and
/// area weights cover only the rectangle, including odd right/bottom edges.
///
/// # Errors
/// Invalid or overflowing rectangle, invalid policy, unsupported pixels,
/// memory/work refusal or cancellation. No partial input is returned.
pub fn prepare_gray_model_region_input(
    image: &LinearRgbaView<'_>,
    rectangle: [u32; 4],
    policy: ModelInputPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<GrayModelInput, ModelInputError> {
    prepare_region(
        image,
        rectangle,
        policy,
        ModelInputProjection::OpaqueSdr,
        budget,
        &cancel,
    )
}

/// Prepare a whole image with an explicit proposal-only projection.
/// Original normalized pixels are not modified. Alpha/HDR confirmation requires
/// a separate original-RGBA verifier; existing rank confirmation is opaque SDR.
/// No full-resolution projection or grayscale scratch is allocated.
///
/// # Errors
/// Invalid projection/policy, allocation/read limits, unsupported pixels or cancellation.
pub fn prepare_gray_model_input_with_projection(
    image: &LinearRgbaView<'_>,
    policy: ModelInputPolicy,
    projection: ModelInputProjection,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<GrayModelInput, ModelInputError> {
    let (w, h) = image.dimensions();
    prepare_gray_model_region_input_with_projection(image, [0, 0, w, h], policy, projection, budget, cancel)
}

/// Project only the supplied original rectangle before area reduction.
/// Projection parameters are validated before allocation. The full source
/// pixel cap still applies; crop origins must be retained by the caller.
///
/// # Errors
/// Invalid rectangle/projection/policy, allocation/read limits or cancellation.
pub fn prepare_gray_model_region_input_with_projection(
    image: &LinearRgbaView<'_>,
    rectangle: [u32; 4],
    policy: ModelInputPolicy,
    projection: ModelInputProjection,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<GrayModelInput, ModelInputError> {
    prepare_region(image, rectangle, policy, projection, budget, &cancel)
}

fn prepare_region(
    image: &LinearRgbaView<'_>,
    [ox, oy, width, height]: [u32; 4],
    policy: ModelInputPolicy,
    projection: ModelInputProjection,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<GrayModelInput, ModelInputError> {
    let checkpoint = || {
        if cancel() {
            Err(ModelInputError::Cancelled)
        } else {
            Ok(())
        }
    };
    checkpoint()?;
    let projection = projection.validate()?;
    let (full_width, full_height) = image.dimensions();
    if policy.max_side < 8
        || width == 0
        || height == 0
        || ox.checked_add(width).is_none_or(|end| end > full_width)
        || oy.checked_add(height).is_none_or(|end| end > full_height)
    {
        return Err(ModelInputError::Invalid);
    }
    let source_pixels = u64::from(width) * u64::from(height);
    if u64::from(full_width) * u64::from(full_height) > policy.max_source_pixels {
        return Err(ModelInputError::Budget);
    }
    let longest = width.max(height);
    let side = policy.max_side.min(longest);
    let dimension = |d: u32| -> Result<u32, ModelInputError> {
        let scaled = u64::from(d) * u64::from(side) / u64::from(longest);
        u32::try_from((scaled / 8 * 8).max(8)).map_err(|_| ModelInputError::Budget)
    };
    let dimensions = [dimension(width)?, dimension(height)?];
    let count = u64::from(dimensions[0]) * u64::from(dimensions[1]);
    if count > policy.max_output_pixels {
        return Err(ModelInputError::Budget);
    }
    let mut output = budget
        .try_buffer(usize::try_from(count).map_err(|_| ModelInputError::Budget)?, 0u8)
        .map_err(|_| ModelInputError::Budget)?;
    let encode = |value: f64| {
        if value <= 0.0031308 {
            12.92 * value
        } else {
            1.055 * value.powf(1. / 2.4) - 0.055
        }
    };
    let mut reads = policy.max_pixel_reads;
    let dw = u64::from(dimensions[0]);
    let dh = u64::from(dimensions[1]);
    for y in 0..dh {
        let top = y * u64::from(height);
        let bottom = (y + 1) * u64::from(height);
        for x in 0..dw {
            checkpoint()?;
            let left = x * u64::from(width);
            let right = (x + 1) * u64::from(width);
            let mut sum = 0.0;
            for sy in top / dh..bottom.div_ceil(dh) {
                let wy = bottom.min((sy + 1) * dh) - top.max(sy * dh);
                for sx in left / dw..right.div_ceil(dw) {
                    checkpoint()?;
                    reads = reads.checked_sub(1).ok_or(ModelInputError::Budget)?;
                    let rgba = image
                        .rgba(
                            ox + u32::try_from(sx).map_err(|_| ModelInputError::Invalid)?,
                            oy + u32::try_from(sy).map_err(|_| ModelInputError::Invalid)?,
                        )
                        .ok_or(ModelInputError::Invalid)?;
                    let rgb = projection.rgb(rgba)?;
                    let wx = right.min((sx + 1) * dw) - left.max(sx * dw);
                    let luminance = 0.299 * encode(rgb[0]) + 0.587 * encode(rgb[1]) + 0.114 * encode(rgb[2]);
                    sum += luminance * (wx as f64) * (wy as f64);
                }
            }
            output[usize::try_from(y * dw + x).map_err(|_| ModelInputError::Budget)?] =
                (sum / (source_pixels as f64) * 255.).round().clamp(0., 255.) as u8;
        }
    }
    checkpoint()?;
    let result = GrayModelInput {
        dimensions,
        original: [width, height],
        pixels: output.freeze(),
    };
    checkpoint()?;
    Ok(result)
}
