use crate::CacheKey;
use crate::color::Color;
use crate::pattern::Pattern;
use crate::util::hash128;
use crate::x_object::ImageXObject;
use hayro_syntax::object::Stream;
use kurbo::{BezPath, Cap, Join};
use smallvec::{SmallVec, smallvec};

/// A clip path.
#[derive(Debug, Clone)]
pub struct ClipPath {
    /// The clipping path.
    pub path: BezPath,
    /// The fill rule.
    pub fill: FillRule,
}

impl CacheKey for ClipPath {
    fn cache_key(&self) -> u128 {
        hash128(&(&self.path.to_svg(), &self.fill))
    }
}

/// A stencil image.
pub struct StencilImage<'a, 'b> {
    pub(crate) paint: Paint<'a>,
    pub(crate) image_xobject: ImageXObject<'b>,
}

impl<'a, 'b> StencilImage<'a, 'b> {
    /// Perform some operation with the stencil data of the image.
    ///
    /// The second argument allows you to give the image decoder a hint for
    /// what resolution of the image you want to have. Note that this does not
    /// mean that the resulting image will have that dimension. Instead, it allows
    /// the image decoder to extract a lower-resolution version of the image in
    /// certain cases.
    pub fn with_stencil(
        &self,
        func: impl FnOnce(LumaData, &Paint<'a>),
        target_dimension: Option<(u32, u32)>,
    ) {
        self.with_stencil_and_cancel(func, target_dimension, &|| false);
    }

    /// Process stencil data with cooperative cancellation. Cancelled data is not published.
    pub fn with_stencil_and_cancel(
        &self,
        func: impl FnOnce(LumaData, &Paint<'a>),
        target_dimension: Option<(u32, u32)>,
        cancelled: &dyn Fn() -> bool,
    ) {
        if let Some(decoded) = self.image_xobject.decoded_mask(target_dimension, cancelled) {
            if cancelled() { return; }
            func(decoded.luma, &self.paint);
        }
    }

    // These are hidden since clients are supposed to call get the
    // width/height from `LumaData` instead.
    #[doc(hidden)]
    pub fn width(&self) -> u32 {
        self.image_xobject.width()
    }

    #[doc(hidden)]
    pub fn height(&self) -> u32 {
        self.image_xobject.height()
    }
}

impl CacheKey for StencilImage<'_, '_> {
    fn cache_key(&self) -> u128 {
        self.image_xobject.cache_key()
    }
}

/// A raster image.
pub struct RasterImage<'a>(pub(crate) ImageXObject<'a>);

/// Native alpha stage required by an image declaration.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum NativeRasterMaskKind {
    /// Stencil image with paint supplied externally.
    Stencil,
    /// Separate grayscale soft-mask stream.
    Soft,
    /// Soft-mask stream with preblended color requiring Matte recovery.
    SoftMatte,
    /// Separate explicit binary-mask stream.
    Explicit,
    /// Transparent ranges in original integer components.
    ColorKey,
    /// Alpha stored inside the compressed image.
    Embedded,
    /// Unsupported or malformed mask declaration.
    Unsupported,
}

/// Reason the original-component image stage could not complete.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum NativeRasterComponentsError {
    /// Cooperative cancellation was observed.
    Cancelled,
    /// Native alpha-mask processing is required.
    Mask(NativeRasterMaskKind),
    /// Native image transfer-function processing is required.
    TransferFunction,
    /// Output buffer or metadata admission was refused.
    Admission,
    /// Stream decompression or image metadata could not be decoded.
    Decode,
    /// Decoder returned a scaled image; full source coordinates are required.
    Scaled,
    /// Component count, length, Decode array or sample data was invalid.
    ComponentData,
}

/// Admitted original float alpha samples from a soft-mask image stream.
pub struct NativeRasterAlpha {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) interpolate: bool,
    pub(crate) samples: crate::native_image_samples::NativeComponentSamples,
    pub(crate) _credit: Box<dyn std::any::Any + Send + Sync>,
}
impl NativeRasterAlpha {
    /// Exact mask width and height, which may differ from the source image.
    pub fn dimensions(&self) -> (u32,u32) { (self.width,self.height) }
    /// Original alpha values in [0,1], before display quantization.
    pub fn samples(&self) -> &[f32] { self.samples.samples() }
    /// The mask image interpolation preference.
    pub fn interpolate(&self) -> bool { self.interpolate }
}

/// Original decoded image components before display color conversion.
/// Supported soft-mask alpha is retained separately from source color samples.
/// Other masks and transfer functions require additional native stages.
pub struct NativeRasterComponents {
    pub(crate) color_space: crate::color::ColorSpace,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) interpolate: bool,
    pub(crate) alpha: Option<NativeRasterAlpha>,
    pub(crate) samples: crate::native_image_samples::NativeComponentSamples,
    pub(crate) _credit: Box<dyn std::any::Any + Send + Sync>,
}
impl NativeRasterComponents {
    /// Original resolved color coordinates.
    pub fn color_space(&self) -> &crate::color::ColorSpace { &self.color_space }
    /// Exact decoded width and height.
    pub fn dimensions(&self) -> (u32,u32) { (self.width,self.height) }
    /// Interleaved float source samples after the PDF Decode array.
    pub fn samples(&self) -> &[f32] { self.samples.samples() }
    /// Native soft-mask data, when the image has a supported soft mask.
    pub fn alpha(&self) -> Option<&NativeRasterAlpha> { self.alpha.as_ref() }
    /// Image interpolation preference.
    pub fn interpolate(&self) -> bool { self.interpolate }
}

impl RasterImage<'_> {
    /// Decode original float components with output-buffer admission.
    /// Filters/stream decompression are managed separately. Soft-mask streams
    /// without Matte are supported; other masks and transfer functions refuse.
    pub fn native_components(
        &self, cancelled: &dyn Fn() -> bool,
        admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    ) -> Option<NativeRasterComponents> { self.native_components_checked(cancelled,admit).ok() }

    /// Decode original components and preserve the reason for any refusal.
    pub fn native_components_checked(
        &self, cancelled: &dyn Fn() -> bool,
        admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    ) -> Result<NativeRasterComponents,NativeRasterComponentsError> { self.0.native_components(cancelled,admit) }

    /// Perform some operation with the RGB and alpha channel of the image.
    ///
    /// The second argument allows you to give the image decoder a hint for
    /// what resolution of the image you want to have. Note that this does not
    /// mean that the resulting image will have that dimension. Instead, it allows
    /// the image decoder to extract a lower-resolution version of the image in
    /// certain cases.
    pub fn with_rgba(
        &self,
        func: impl FnOnce(ImageData, Option<LumaData>),
        target_dimension: Option<(u32, u32)>,
    ) {
        self.with_rgba_and_cancel(func, target_dimension, &|| false)
    }

    /// Decode with cooperative cancellation; no partial image reaches the callback.
    pub fn with_rgba_and_cancel(&self, func: impl FnOnce(ImageData, Option<LumaData>), target_dimension: Option<(u32, u32)>, cancelled: &dyn Fn() -> bool) {
        if let Some(decoded) = self.0.decoded_raster(target_dimension, cancelled) {
            if !cancelled() { func(decoded.image, decoded.alpha); }
        }
    }

    /// Resolved source coordinates before conversion to compatibility RGB.
    /// Stencil images have no intrinsic color space.
    pub fn color_space(&self) -> Option<&crate::color::ColorSpace> { self.0.color_space() }

    /// Return the underlying stream object.
    ///
    /// This allows you to get access to the raw encoded image data, without doing any decoding.
    pub fn stream(&self) -> &Stream<'_> {
        self.0.stream()
    }

    // These are hidden since clients are supposed to call get the
    // width/height from `LumaData` instead.
    #[doc(hidden)]
    pub fn width(&self) -> u32 {
        self.0.width()
    }

    #[doc(hidden)]
    pub fn height(&self) -> u32 {
        self.0.height()
    }
}

impl CacheKey for RasterImage<'_> {
    fn cache_key(&self) -> u128 {
        self.0.cache_key()
    }
}

/// A type of image.
pub enum Image<'a, 'b> {
    /// A stencil image.
    Stencil(StencilImage<'a, 'b>),
    /// A normal raster image.
    Raster(RasterImage<'b>),
}

impl Image<'_, '_> {
    // These are hidden since clients are supposed to call get the
    // width/height from `LumaData/RgbData` instead.
    #[doc(hidden)]
    pub fn width(&self) -> u32 {
        match self {
            Image::Stencil(s) => s.width(),
            Image::Raster(r) => r.width(),
        }
    }

    // These are hidden since clients are supposed to call get the
    // width/height from `LumaData/RgbData` instead.
    #[doc(hidden)]
    pub fn height(&self) -> u32 {
        match self {
            Image::Stencil(s) => s.height(),
            Image::Raster(r) => r.height(),
        }
    }
}

impl CacheKey for Image<'_, '_> {
    fn cache_key(&self) -> u128 {
        match self {
            Image::Stencil(i) => i.cache_key(),
            Image::Raster(i) => i.cache_key(),
        }
    }
}

/// A structure holding 3-channel RGB data.
#[derive(Clone)]
pub struct RgbData {
    /// The actual data. It is guaranteed to have the length width * height * 3.
    pub data: Vec<u8>,
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
    /// Whether the image should be interpolated.
    pub interpolate: bool,
    /// Additional scaling factors to apply to the image.
    ///
    /// In most cases, those factors will just be 1.0, and you can
    /// ignore them. There are two situations in which they will not be equal
    /// to 1:
    /// 1) The PDF provided wrong metadata about the width/height of the image,
    ///    which needs to be corrected
    /// 2) A lower resolution of the image was requested, in which case it needs
    ///    to be scaled up so that it still covers the same area.
    ///
    /// The first number indicates the x scaling factor, the second number the
    /// y scaling factor.
    pub scale_factors: (f32, f32),
}

/// A structure holding 1-channel luma data.
#[derive(Clone)]
pub struct LumaData {
    /// The actual data. It is guaranteed to have the length width * height.
    pub data: Vec<u8>,
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
    /// Whether the image should be interpolated.
    pub interpolate: bool,
    /// Additional scaling factors to apply to the image.
    ///
    /// In most cases, those factors will just be 1.0, and you can
    /// ignore them. There are two situations in which they will not be equal
    /// to 1:
    /// 1) The PDF provided wrong metadata about the width/height of the image,
    ///    which needs to be corrected
    /// 2) A lower resolution of the image was requested, in which case it needs
    ///    to be scaled up so that it still covers the same area.
    ///
    /// The first number indicates the x scaling factor, the second number the
    /// y scaling factor.
    pub scale_factors: (f32, f32),
}

/// The color data of a raster image, either 3-channel RGB or 1-channel luma.
#[derive(Clone)]
pub enum ImageData {
    /// 3-channel RGB data.
    Rgb(RgbData),
    /// 1-channel grayscale data.
    Luma(LumaData),
}

impl ImageData {
    /// The width of the image.
    pub fn width(&self) -> u32 {
        match self {
            Self::Rgb(d) => d.width,
            Self::Luma(d) => d.width,
        }
    }

    /// The height of the image.
    pub fn height(&self) -> u32 {
        match self {
            Self::Rgb(d) => d.height,
            Self::Luma(d) => d.height,
        }
    }

    /// Whether the image should be interpolated.
    pub fn interpolate(&self) -> bool {
        match self {
            Self::Rgb(d) => d.interpolate,
            Self::Luma(d) => d.interpolate,
        }
    }

    /// The scaling factors of the image.
    pub fn scale_factors(&self) -> (f32, f32) {
        match self {
            Self::Rgb(d) => d.scale_factors,
            Self::Luma(d) => d.scale_factors,
        }
    }
}

/// A type of paint.
#[derive(Clone, Debug)]
pub enum Paint<'a> {
    /// A solid RGBA color.
    Color(Color),
    /// A PDF pattern.
    Pattern(Box<Pattern<'a>>),
}

impl CacheKey for Paint<'_> {
    fn cache_key(&self) -> u128 {
        match self {
            Paint::Color(c) => {
                // TODO: We should actually cache the color with color space etc., not just the
                // RGBA8 version.
                hash128(&c.to_rgba().to_rgba8())
            }
            Paint::Pattern(p) => p.cache_key(),
        }
    }
}

/// The draw mode that should be used for a path.
#[derive(Clone, Debug)]
pub enum PathDrawMode {
    /// Draw using a fill.
    Fill(FillRule),
    /// Draw using a stroke.
    Stroke(StrokeProps),
}

/// The draw mode that should be used for a glyph.
#[derive(Clone, Debug)]
pub enum GlyphDrawMode {
    /// Draw using a fill.
    Fill,
    /// Draw using a stroke.
    Stroke(StrokeProps),
    /// Invisible text (for text extraction but not visual rendering).
    Invisible,
}

/// Stroke properties.
#[derive(Clone, Debug)]
pub struct StrokeProps {
    /// The line width.
    pub line_width: f32,
    /// The line cap.
    pub line_cap: Cap,
    /// The line join.
    pub line_join: Join,
    /// The miter limit.
    pub miter_limit: f32,
    /// The dash array.
    pub dash_array: SmallVec<[f32; 4]>,
    /// The dash offset.
    pub dash_offset: f32,
}

impl Default for StrokeProps {
    fn default() -> Self {
        Self {
            line_width: 1.0,
            line_cap: Cap::Butt,
            line_join: Join::Miter,
            miter_limit: 10.0,
            dash_array: smallvec![],
            dash_offset: 0.0,
        }
    }
}

/// A fill rule.
#[derive(Clone, Debug, Copy, Hash, PartialEq, Eq)]
pub enum FillRule {
    /// Non-zero filling.
    NonZero,
    /// Even-odd filling.
    EvenOdd,
}

/// A blend mode.
#[derive(Clone, Debug, Copy, Hash, PartialEq, Eq, Default)]
pub enum BlendMode {
    /// Normal blend mode (default).
    #[default]
    Normal,
    /// Multiply blend mode.
    Multiply,
    /// Screen blend mode.
    Screen,
    /// Overlay blend mode.
    Overlay,
    /// Darken blend mode.
    Darken,
    /// Lighten blend mode.
    Lighten,
    /// `ColorDodge` blend mode.
    ColorDodge,
    /// `ColorBurn` blend mode.
    ColorBurn,
    /// `HardLight` blend mode.
    HardLight,
    /// `SoftLight` blend mode.
    SoftLight,
    /// Difference blend mode.
    Difference,
    /// Exclusion blend mode.
    Exclusion,
    /// Hue blend mode.
    Hue,
    /// Saturation blend mode.
    Saturation,
    /// Color blend mode.
    Color,
    /// Luminosity blend mode.
    Luminosity,
}
