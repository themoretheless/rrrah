//! PDF colors and color spaces.

use crate::cache::{Cache, CacheKey};
use crate::function::Function;
use hayro_syntax::object;
use hayro_syntax::object::Array;
use hayro_syntax::object::Dict;
use hayro_syntax::object::Name;
use hayro_syntax::object::Object;
use hayro_syntax::object::Stream;
use hayro_syntax::object::dict::keys::*;
use moxcms::{
    ColorProfile, DataColorSpace, Layout, Transform8BitExecutor, TransformF32Executor,
    TransformOptions, Xyzd,
};
use smallvec::{SmallVec, ToSmallVec, smallvec};
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::sync::{Arc, LazyLock, OnceLock};

/// A storage for the components of colors.
pub type ColorComponents = SmallVec<[f32; 4]>;

/// An RGB color with an alpha channel.
#[derive(Debug, Copy, Clone)]
pub struct AlphaColor {
    components: [f32; 4],
}

impl AlphaColor {
    /// A black color.
    pub const BLACK: Self = Self::new([0., 0., 0., 1.]);

    /// A transparent color.
    pub const TRANSPARENT: Self = Self::new([0., 0., 0., 0.]);

    /// A white color.
    pub const WHITE: Self = Self::new([1., 1., 1., 1.]);

    /// Create a new color from the given components.
    pub const fn new(components: [f32; 4]) -> Self {
        Self { components }
    }

    /// Create a new color from RGB8 values.
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        let components = [u8_to_f32(r), u8_to_f32(g), u8_to_f32(b), 1.];
        Self::new(components)
    }

    /// Return the color as premulitplied RGBF32.
    pub fn premultiplied(&self) -> [f32; 4] {
        [
            self.components[0] * self.components[3],
            self.components[1] * self.components[3],
            self.components[2] * self.components[3],
            self.components[3],
        ]
    }

    /// Create a new color from RGBA8 values.
    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        let components = [u8_to_f32(r), u8_to_f32(g), u8_to_f32(b), u8_to_f32(a)];
        Self::new(components)
    }

    /// Return the color as RGBA8.
    pub fn to_rgba8(&self) -> [u8; 4] {
        [
            (self.components[0] * 255.0 + 0.5) as u8,
            (self.components[1] * 255.0 + 0.5) as u8,
            (self.components[2] * 255.0 + 0.5) as u8,
            (self.components[3] * 255.0 + 0.5) as u8,
        ]
    }

    /// Return the components of the color as RGBF32.
    pub fn components(&self) -> [f32; 4] {
        self.components
    }
}

const fn u8_to_f32(x: u8) -> f32 {
    x as f32 * (1.0 / 255.0)
}

#[derive(Debug, Clone)]
pub(crate) enum ColorSpaceType {
    DeviceCmyk,
    DeviceGray,
    DeviceRgb,
    Pattern(ColorSpace),
    Indexed(Indexed),
    ICCBased(ICCProfile),
    CalGray(CalGray),
    CalRgb(CalRgb),
    Lab(Lab),
    Separation(Separation),
    DeviceN(DeviceN),
}

impl ColorSpaceType {
    fn new(object: Object<'_>, cache: &Cache) -> Option<Self> {
        Self::new_inner(object, cache)
    }

    fn new_inner(object: Object<'_>, cache: &Cache) -> Option<Self> {
        if let Object::Name(name) = object {
            return Self::new_from_name(&name);
        } else if let Object::Array(color_array) = object {
            let mut iter = color_array.flex_iter();
            let name = iter.next::<Name<'_>>()?;

            match name.deref() {
                ICC_BASED => {
                    let icc_stream = iter.next::<Stream<'_>>()?;
                    let dict = icc_stream.dict();
                    let num_components = dict.get::<usize>(N)?;

                    return cache.get_or_insert_with(icc_stream.cache_key(), || {
                        if let Some(decoded) = icc_stream.decoded().ok().as_ref() {
                            ICCProfile::new(decoded, num_components)
                                .map(|icc| {
                                    // TODO: For SVG and PNG we can assume that the output color space is
                                    // sRGB. If we ever implement PDF-to-PDF, we probably want to
                                    // let the user pass the native color type and don't make this optimization
                                    // if it's not sRGB.
                                    if icc.is_srgb() {
                                        Self::DeviceRgb
                                    } else {
                                        Self::ICCBased(icc)
                                    }
                                })
                                .or_else(|| {
                                    dict.get::<Object<'_>>(ALTERNATE)
                                        .and_then(|o| Self::new(o, cache))
                                })
                                .or_else(|| match dict.get::<u8>(N) {
                                    Some(1) => Some(Self::DeviceGray),
                                    Some(3) => Some(Self::DeviceRgb),
                                    Some(4) => Some(Self::DeviceCmyk),
                                    _ => None,
                                })
                        } else {
                            None
                        }
                    });
                }
                CALCMYK => return Some(Self::DeviceCmyk),
                CALGRAY => {
                    let cal_dict = iter.next::<Dict<'_>>()?;
                    return Some(Self::CalGray(CalGray::new(&cal_dict)?));
                }
                CALRGB => {
                    let cal_dict = iter.next::<Dict<'_>>()?;
                    return Some(Self::CalRgb(CalRgb::new(&cal_dict)?));
                }
                DEVICE_RGB | RGB => return Some(Self::DeviceRgb),
                DEVICE_GRAY | G => return Some(Self::DeviceGray),
                DEVICE_CMYK | CMYK => return Some(Self::DeviceCmyk),
                LAB => {
                    let lab_dict = iter.next::<Dict<'_>>()?;
                    return Some(Self::Lab(Lab::new(&lab_dict)?));
                }
                INDEXED | I => {
                    return Some(Self::Indexed(Indexed::new(&color_array, cache)?));
                }
                SEPARATION => {
                    return Some(Self::Separation(Separation::new(&color_array, cache)?));
                }
                DEVICE_N => {
                    return Some(Self::DeviceN(DeviceN::new(&color_array, cache)?));
                }
                PATTERN => {
                    let _ = iter.next::<Name<'_>>();
                    let cs = iter
                        .next::<Object<'_>>()
                        .and_then(|o| ColorSpace::new(o, cache))
                        .unwrap_or(ColorSpace::device_rgb());
                    return Some(Self::Pattern(cs));
                }
                _ => {
                    warn!("unsupported color space: {}", name.as_str());
                    return None;
                }
            }
        }

        None
    }

    fn new_from_name(name: &Name<'_>) -> Option<Self> {
        match name.deref() {
            DEVICE_RGB | RGB => Some(Self::DeviceRgb),
            DEVICE_GRAY | G => Some(Self::DeviceGray),
            DEVICE_CMYK | CMYK => Some(Self::DeviceCmyk),
            CALCMYK => Some(Self::DeviceCmyk),
            PATTERN => Some(Self::Pattern(ColorSpace::device_rgb())),
            _ => None,
        }
    }
}

/// Native component layout for a legal PDF blending color space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendingModel {
    /// One grayscale component.
    Gray,
    /// Red, green and blue components.
    Rgb,
    /// Cyan, magenta, yellow and black components.
    Cmyk,
}

/// A PDF color space.
#[derive(Debug, Clone)]
pub struct ColorSpace(Arc<ColorSpaceType>, u128);

/// An explicit floating-point conversion between two ICC blending spaces.
/// Compile once and retain outside the pixel loop. Construction may allocate.
pub struct BlendingTransform {
    executor: Arc<TransformF32Executor>,
    source_components: usize,
    destination_components: usize,
}

impl BlendingTransform {
    /// Convert one unpremultiplied native sample without display quantization.
    /// Invalid component counts or nonfinite/out-of-range samples are rejected.
    pub fn convert(&self, source: &[f32]) -> Option<[f32; 4]> {
        if source.len() != self.source_components
            || source.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return None;
        }
        let mut destination = [0.0; 4];
        self.executor.transform(source, &mut destination[..self.destination_components]).ok()?;
        if destination[..self.destination_components]
            .iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return None;
        }
        Some(destination)
    }
}

impl ColorSpace {
    /// Convert bare RGB to bare CMYK with explicit graphics-state BG/UCR.
    /// Defaults/unresolved functions and unsupported bounded evaluators refuse.
    pub fn rgb_sample_to_cmyk_with(
        &self, destination: &Self, input: &[f32], functions: &DeviceConversionFunctions,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<[f32; 4]> {
        if cancelled() || !matches!(self.0.as_ref(), ColorSpaceType::DeviceRgb)
            || !matches!(destination.0.as_ref(), ColorSpaceType::DeviceCmyk)
            || input.len() != 3
            || input.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        { return None; }
        let c = 1.0 - input[0];
        let m = 1.0 - input[1];
        let y = 1.0 - input[2];
        let k = c.min(m).min(y);
        let DeviceConversionFunction::Function(bg) = &functions.black_generation else { return None; };
        let DeviceConversionFunction::Function(ucr) = &functions.undercolor_removal else { return None; };
        let black = bg.eval_scalar_bounded(k, cancelled)?;
        let removal = ucr.eval_scalar_bounded(k, cancelled)?;
        if cancelled() { return None; }
        Some([(c-removal).clamp(0.0,1.0), (m-removal).clamp(0.0,1.0),
            (y-removal).clamp(0.0,1.0), black.clamp(0.0,1.0)])
    }
    /// PDF 32000-1 section10.3 device conversions to Gray or Gray-to-CMYK.
    /// RGB-to-CMYK is refused: its black-generation/undercolor-removal
    /// functions belong to graphics state and must be supplied explicitly.
    pub fn device_sample_to_gray_or_cmyk(&self, destination: &Self, input: &[f32]) -> Option<[f32; 4]> {
        if input.len() != usize::from(self.num_components())
            || input.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        { return None; }
        let mut out = [0.0; 4];
        match (self.0.as_ref(), destination.0.as_ref()) {
            (ColorSpaceType::DeviceGray, ColorSpaceType::DeviceCmyk) => out[3] = 1.0 - input[0],
            (source, ColorSpaceType::DeviceGray) => {
                out[0] = match source {
                    ColorSpaceType::DeviceGray => input[0],
                    ColorSpaceType::DeviceRgb => 0.3 * input[0] + 0.59 * input[1] + 0.11 * input[2],
                    ColorSpaceType::DeviceCmyk => 1.0 - (0.3 * input[0] + 0.59 * input[1] + 0.11 * input[2] + input[3]).min(1.0),
                    _ => return None,
                };
            }
            _ => return None,
        }
        Some(out)
    }
    /// Evaluate the renderer's calibrated-space display policy in float RGB.
    /// Destination must be bare DeviceRGB. This preserves the current policy;
    /// it does not qualify calibrated group-boundary conformance.
    pub fn calibrated_sample_to_rgb(&self, destination: &Self, input: &[f32]) -> Option<[f32; 4]> {
        if !matches!(destination.0.as_ref(), ColorSpaceType::DeviceRgb)
            || input.len() != usize::from(self.num_components())
            || input.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        { return None; }
        let rgb = match self.0.as_ref() {
            ColorSpaceType::CalRgb(cal) => cal.rgb_float(input)?,
            ColorSpaceType::CalGray(cal) => {
                if !cal.gamma.is_finite() || cal.gamma <= 0.0
                    || cal.white_point.iter().any(|v| !v.is_finite() || *v <= 0.0)
                    || cal.black_point.iter().any(|v| !v.is_finite())
                { return None; }
                let luminance = cal.white_point[1] * input[0].powf(cal.gamma);
                if !luminance.is_finite() { return None; }
                let gray = (295.8 * luminance.powf(0.333_333_34) - 40.8).max(0.0) / 255.0;
                [gray.clamp(0.0, 1.0); 3]
            }
            _ => return None,
        };
        Some([rgb[0], rgb[1], rgb[2], 0.0])
    }
    /// Convert a bare device-space sample to bare DeviceRGB without RGBA8.
    /// CMYK uses this renderer's existing CGATS/hybrid display policy.
    /// This is not an ICC-coordinate equivalence or a calibrated-space transform.
    pub fn device_sample_to_rgb(&self, destination: &Self, input: &[f32]) -> Option<[f32; 4]> {
        if !matches!(destination.0.as_ref(), ColorSpaceType::DeviceRgb)
            || input.len() != usize::from(self.num_components())
            || input.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return None;
        }
        let mut output = [0.0; 4];
        match self.0.as_ref() {
            ColorSpaceType::DeviceGray => output[..3].fill(input[0]),
            ColorSpaceType::DeviceRgb => output[..3].copy_from_slice(input),
            ColorSpaceType::DeviceCmyk => DEVICE_CMYK_FLOAT.transform(input, &mut output[..3]).ok()?,
            _ => return None,
        }
        if output[..3].iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
            return None;
        }
        Some(output)
    }
    /// Compile a direct ICC-to-ICC native blending conversion.
    /// Device/calibrated spaces require an explicit profile policy and are
    /// refused here rather than assigning an implicit display profile.
    pub fn blending_transform_to(&self, destination: &Self) -> Option<BlendingTransform> {
        self.blending_model()?;
        destination.blending_model()?;
        let (ColorSpaceType::ICCBased(source), ColorSpaceType::ICCBased(target)) =
            (self.0.as_ref(), destination.0.as_ref()) else { return None; };
        let executor = source.0.src_profile.create_transform_f32(
            source.0.src_layout,
            &target.0.src_profile,
            target.0.src_layout,
            TransformOptions { prefer_fixed_point: false, ..TransformOptions::default() },
        ).ok()?;
        Some(BlendingTransform {
            executor,
            source_components: source.0.number_components,
            destination_components: target.0.number_components,
        })
    }
    /// Native layout; special/Lab spaces require conversion before blending.
    pub fn blending_model(&self) -> Option<BlendingModel> {
        match self.0.as_ref() {
            ColorSpaceType::DeviceGray | ColorSpaceType::CalGray(_) => Some(BlendingModel::Gray),
            ColorSpaceType::DeviceRgb | ColorSpaceType::CalRgb(_) => Some(BlendingModel::Rgb),
            ColorSpaceType::DeviceCmyk => Some(BlendingModel::Cmyk),
            ColorSpaceType::ICCBased(profile) => match profile.0.src_profile.color_space {
                DataColorSpace::Gray => Some(BlendingModel::Gray),
                DataColorSpace::Rgb => Some(BlendingModel::Rgb),
                DataColorSpace::Cmyk => Some(BlendingModel::Cmyk),
                _ => None,
            },
            _ => None,
        }
    }
    /// True only when native components may be copied without color conversion.
    /// Independently loaded ICC profiles conservatively require a transform.
    pub fn shares_blending_coordinates(&self, other: &Self) -> bool {
        if self.blending_model().is_none() || self.blending_model() != other.blending_model() {
            return false;
        }
        if Arc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        match (self.0.as_ref(), other.0.as_ref()) {
            (ColorSpaceType::DeviceGray, ColorSpaceType::DeviceGray)
            | (ColorSpaceType::DeviceRgb, ColorSpaceType::DeviceRgb)
            | (ColorSpaceType::DeviceCmyk, ColorSpaceType::DeviceCmyk) => true,
            (ColorSpaceType::CalGray(a), ColorSpaceType::CalGray(b)) => a == b,
            (ColorSpaceType::CalRgb(a), ColorSpaceType::CalRgb(b)) => a == b,
            (ColorSpaceType::ICCBased(a), ColorSpaceType::ICCBased(b)) => Arc::ptr_eq(&a.0, &b.0),
            _ => false,
        }
    }
    /// Retained capacity of a plain device color-space Arc; profiles and
    /// calibrated/spot spaces require a separately admitted capture path.
    pub fn native_device_retained_capacity(&self) -> Option<usize> {
        self.is_device().then_some(size_of::<ColorSpaceType>() + 2*size_of::<usize>())
    }
    pub(crate) fn is_device(&self) -> bool {
        matches!(self.0.as_ref(), ColorSpaceType::DeviceGray | ColorSpaceType::DeviceRgb | ColorSpaceType::DeviceCmyk)
    }
    pub(crate) fn resource_key(&self) -> u128 { self.1 }
    /// Create a new color space from the given object.
    pub(crate) fn new(object: Object<'_>, cache: &Cache) -> Option<Self> {
        let key = object.cache_key();
        Some(Self(Arc::new(ColorSpaceType::new(object, cache)?), key))
    }

    /// Create a new color space from the name.
    pub(crate) fn new_from_name(name: &Name<'_>) -> Option<Self> {
        ColorSpaceType::new_from_name(name).map(|c| Self(Arc::new(c), 0))
    }

    /// Return the device gray color space.
    pub(crate) fn device_gray() -> Self {
        Self(Arc::new(ColorSpaceType::DeviceGray), 0)
    }

    /// Return the device RGB color space.
    pub(crate) fn device_rgb() -> Self {
        Self(Arc::new(ColorSpaceType::DeviceRgb), 0)
    }

    /// Return the device CMYK color space.
    pub(crate) fn device_cmyk() -> Self {
        Self(Arc::new(ColorSpaceType::DeviceCmyk), 0)
    }

    /// Return the pattern color space.
    pub(crate) fn pattern() -> Self {
        Self(Arc::new(ColorSpaceType::Pattern(Self::device_gray())), 0)
    }

    pub(crate) fn pattern_cs(&self) -> Option<Self> {
        match self.0.as_ref() {
            ColorSpaceType::Pattern(cs) => Some(cs.clone()),
            _ => None,
        }
    }

    /// Return `true` if the current color space is the pattern color space.
    pub(crate) fn is_pattern(&self) -> bool {
        matches!(self.0.as_ref(), ColorSpaceType::Pattern(_))
    }

    /// Resolve one original sample into blending coordinates without RGB8.
    /// Indexed palettes are expanded before interpolation. Unsupported base
    /// spaces (e.g. Lab or spot coordinates) require a separate native transform.
    pub fn native_sample_coordinates(&self, input: &[f32]) -> Option<(&Self,[f32;4])> {
        if input.iter().any(|value| !value.is_finite()) { return None; }
        let (space,values)=if let ColorSpaceType::Indexed(indexed)=self.0.as_ref() {
            if input.len()!=1 { return None; }
            let index=(input[0].clamp(0.,f32::from(indexed.hival))+0.5) as usize;
            (indexed.base.as_ref(),indexed.values.get(index)?.as_slice())
        } else { (self,input) };
        space.blending_model()?;
        let count=usize::from(space.num_components());
        if count>4 || values.len()!=count || values.iter().any(|value| !value.is_finite() || !(0. ..=1.).contains(value)) { return None; }
        let mut out=[0.;4];out[..count].copy_from_slice(values);Some((space,out))
    }

    /// Return `true` if the current color space is an indexed color space.
    pub(crate) fn is_indexed(&self) -> bool {
        matches!(self.0.as_ref(), ColorSpaceType::Indexed(_))
    }

    /// Get the default decode array for the color space.
    pub(crate) fn default_decode_arr(&self, n: f32) -> SmallVec<[(f32, f32); 4]> {
        match self.0.as_ref() {
            ColorSpaceType::DeviceCmyk => smallvec![(0.0, 1.0), (0.0, 1.0), (0.0, 1.0), (0.0, 1.0)],
            ColorSpaceType::DeviceGray => smallvec![(0.0, 1.0)],
            ColorSpaceType::DeviceRgb => smallvec![(0.0, 1.0), (0.0, 1.0), (0.0, 1.0)],
            ColorSpaceType::ICCBased(i) => smallvec![(0.0, 1.0); i.0.number_components],
            ColorSpaceType::CalGray(_) => smallvec![(0.0, 1.0)],
            ColorSpaceType::CalRgb(_) => smallvec![(0.0, 1.0), (0.0, 1.0), (0.0, 1.0)],
            ColorSpaceType::Lab(l) => smallvec![
                (0.0, 100.0),
                (l.range[0], l.range[1]),
                (l.range[2], l.range[3]),
            ],
            ColorSpaceType::Indexed(_) => smallvec![(0.0, 2.0_f32.powf(n) - 1.0)],
            ColorSpaceType::Separation(_) => smallvec![(0.0, 1.0)],
            ColorSpaceType::DeviceN(d) => smallvec![(0.0, 1.0); d.num_components as usize],
            // Not a valid image color space.
            ColorSpaceType::Pattern(_) => smallvec![(0.0, 1.0)],
        }
    }

    pub(crate) fn inverted_default_decode_arr(&self, n: f32) -> SmallVec<[(f32, f32); 4]> {
        self.default_decode_arr(n)
            .iter()
            .map(|(min, max)| (*max, *min))
            .collect()
    }

    /// Get the initial color of the color space.
    pub(crate) fn initial_color(&self) -> ColorComponents {
        match self.0.as_ref() {
            ColorSpaceType::DeviceCmyk => smallvec![0.0, 0.0, 0.0, 1.0],
            ColorSpaceType::DeviceGray => smallvec![0.0],
            ColorSpaceType::DeviceRgb => smallvec![0.0, 0.0, 0.0],
            ColorSpaceType::ICCBased(icc) => match icc.0.number_components {
                1 => smallvec![0.0],
                3 => smallvec![0.0, 0.0, 0.0],
                4 => smallvec![0.0, 0.0, 0.0, 1.0],
                _ => unreachable!(),
            },
            ColorSpaceType::CalGray(_) => smallvec![0.0],
            ColorSpaceType::CalRgb(_) => smallvec![0.0, 0.0, 0.0],
            ColorSpaceType::Lab(_) => smallvec![0.0, 0.0, 0.0],
            ColorSpaceType::Indexed(_) => smallvec![0.0],
            ColorSpaceType::Separation(_) => smallvec![1.0],
            ColorSpaceType::Pattern(c) => c.initial_color(),
            ColorSpaceType::DeviceN(d) => smallvec![1.0; d.num_components as usize],
        }
    }

    pub(crate) fn is_device_gray(&self) -> bool {
        matches!(self.0.as_ref(), ColorSpaceType::DeviceGray)
    }

    pub(crate) fn valid_blending_space(&self) -> bool { self.blending_model().is_some() }

    /// Get the number of components of the color space.
    /// Number of native components before conversion to display RGB.
    pub fn num_components(&self) -> u8 {
        match self.0.as_ref() {
            ColorSpaceType::DeviceCmyk => 4,
            ColorSpaceType::DeviceGray => 1,
            ColorSpaceType::DeviceRgb => 3,
            ColorSpaceType::ICCBased(icc) => icc.0.number_components as u8,
            ColorSpaceType::CalGray(_) => 1,
            ColorSpaceType::CalRgb(_) => 3,
            ColorSpaceType::Lab(_) => 3,
            ColorSpaceType::Indexed(_) => 1,
            ColorSpaceType::Separation(_) => 1,
            ColorSpaceType::Pattern(p) => p.num_components(),
            ColorSpaceType::DeviceN(d) => d.num_components,
        }
    }

    /// Turn the given component values and opacity into an RGBA color.
    pub fn to_rgba(&self, c: &[f32], opacity: f32, manual_scale: bool) -> AlphaColor {
        self.to_alpha_color(c, opacity, manual_scale)
            .unwrap_or(AlphaColor::BLACK)
    }
}

impl ToRgb for ColorSpace {
    fn prefers_float_samples(&self) -> bool { matches!(self.0.as_ref(), ColorSpaceType::DeviceCmyk) }
    fn convert_f32(&self, input: &[f32], output: &mut [u8], manual_scale: bool) -> Option<()> {
        match self.0.as_ref() {
            ColorSpaceType::DeviceCmyk => {
                if output.len() == 3 {
                    // Vector fills transform one color; avoid a separate RGB heap buffer.
                    let mut rgb = [0.0; 3];
                    DEVICE_CMYK_FLOAT.transform(input, &mut rgb).ok()?;
                    for (value, output) in rgb.iter().zip(output) { *output = f32_to_u8(*value); }
                } else {
                    let mut rgb = vec![0.0; output.len()];
                    DEVICE_CMYK_FLOAT.transform(input, &mut rgb).ok()?;
                    for (value, output) in rgb.iter().zip(output) { *output = f32_to_u8(*value); }
                }
                Some(())
            }
            ColorSpaceType::DeviceGray => {
                for (gray, output) in input.iter().zip(output.chunks_exact_mut(3)) {
                    let gray = f32_to_u8(*gray);
                    output.copy_from_slice(&[gray, gray, gray]);
                }

                Some(())
            }
            ColorSpaceType::DeviceRgb => {
                for (input, output) in input.iter().copied().zip(output) {
                    *output = f32_to_u8(input);
                }

                Some(())
            }
            ColorSpaceType::Pattern(i) => i.convert_f32(input, output, manual_scale),
            ColorSpaceType::Indexed(i) => i.convert_f32(input, output, manual_scale),
            ColorSpaceType::ICCBased(i) => i.convert_f32(input, output, manual_scale),
            ColorSpaceType::CalGray(i) => i.convert_f32(input, output, manual_scale),
            ColorSpaceType::CalRgb(i) => i.convert_f32(input, output, manual_scale),
            ColorSpaceType::Lab(i) => i.convert_f32(input, output, manual_scale),
            ColorSpaceType::Separation(i) => i.convert_f32(input, output, manual_scale),
            ColorSpaceType::DeviceN(i) => i.convert_f32(input, output, manual_scale),
        }
    }

    fn supports_u8(&self) -> bool {
        match self.0.as_ref() {
            ColorSpaceType::DeviceCmyk => true,
            ColorSpaceType::DeviceGray => true,
            ColorSpaceType::DeviceRgb => true,
            ColorSpaceType::Pattern(i) => i.supports_u8(),
            ColorSpaceType::Indexed(i) => i.supports_u8(),
            ColorSpaceType::ICCBased(i) => i.supports_u8(),
            ColorSpaceType::CalGray(i) => i.supports_u8(),
            ColorSpaceType::CalRgb(i) => i.supports_u8(),
            ColorSpaceType::Lab(i) => i.supports_u8(),
            ColorSpaceType::Separation(i) => i.supports_u8(),
            ColorSpaceType::DeviceN(i) => i.supports_u8(),
        }
    }

    fn convert_u8(&self, input: &[u8], output: &mut [u8]) -> Option<()> {
        match self.0.as_ref() {
            ColorSpaceType::DeviceCmyk => {
                if input.len() % 4 != 0 || output.len() != input.len() / 4 * 3 { return None; }
                // Bounded scratch: normalize CMYK8 codes without a whole-image float copy.
                let mut cmyk = [0.0_f32; 256 * 4];
                let mut rgb = [0.0_f32; 256 * 3];
                for (source, destination) in input.chunks(256 * 4).zip(output.chunks_mut(256 * 3)) {
                    for (value, byte) in cmyk.iter_mut().zip(source) { *value = f32::from(*byte) / 255.0; }
                    DEVICE_CMYK_FLOAT.transform(&cmyk[..source.len()], &mut rgb[..destination.len()]).ok()?;
                    for (value, byte) in rgb.iter().zip(destination) { *byte = f32_to_u8(*value); }
                }
                Some(())
            },
            ColorSpaceType::DeviceGray => {
                for (input, output) in input.iter().zip(output.chunks_exact_mut(3)) {
                    output.copy_from_slice(&[*input, *input, *input]);
                }

                Some(())
            }
            ColorSpaceType::DeviceRgb => {
                for (input, output) in input.iter().zip(output.iter_mut()) {
                    *output = *input;
                }

                Some(())
            }
            ColorSpaceType::Pattern(i) => i.convert_u8(input, output),
            ColorSpaceType::Indexed(i) => i.convert_u8(input, output),
            ColorSpaceType::ICCBased(i) => i.convert_u8(input, output),
            ColorSpaceType::CalGray(i) => i.convert_u8(input, output),
            ColorSpaceType::CalRgb(i) => i.convert_u8(input, output),
            ColorSpaceType::Lab(i) => i.convert_u8(input, output),
            ColorSpaceType::Separation(i) => i.convert_u8(input, output),
            ColorSpaceType::DeviceN(i) => i.convert_u8(input, output),
        }
    }

    fn is_none(&self) -> bool {
        match self.0.as_ref() {
            ColorSpaceType::Separation(s) => s.is_none(),
            ColorSpaceType::DeviceN(d) => d.is_none(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CalGray {
    white_point: [f32; 3],
    black_point: [f32; 3],
    gamma: f32,
}

// See <https://github.com/mozilla/pdf.js/blob/06f44916c8936b92f464d337fe3a0a6b2b78d5b4/src/core/colorspace.js#L752>
impl CalGray {
    fn new(dict: &Dict<'_>) -> Option<Self> {
        let white_point = dict.get::<[f32; 3]>(WHITE_POINT).unwrap_or([1.0, 1.0, 1.0]);
        let black_point = dict.get::<[f32; 3]>(BLACK_POINT).unwrap_or([0.0, 0.0, 0.0]);
        let gamma = dict.get::<f32>(GAMMA).unwrap_or(1.0);

        Some(Self {
            white_point,
            black_point,
            gamma,
        })
    }
}

impl ToRgb for CalGray {
    fn convert_f32(&self, input: &[f32], output: &mut [u8], _: bool) -> Option<()> {
        for (input, output) in input.iter().copied().zip(output.chunks_exact_mut(3)) {
            let g = self.gamma;
            let (_xw, yw, _zw) = {
                let wp = self.white_point;
                (wp[0], wp[1], wp[2])
            };
            let (_xb, _yb, _zb) = {
                let bp = self.black_point;
                (bp[0], bp[1], bp[2])
            };

            let a = input;
            let ag = a.powf(g);
            let l = yw * ag;
            let val = (0.0_f32.max(295.8 * l.powf(0.333_333_34) - 40.8) + 0.5) as u8;

            output.copy_from_slice(&[val, val, val]);
        }

        Some(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CalRgb {
    white_point: [f32; 3],
    black_point: [f32; 3],
    matrix: [f32; 9],
    gamma: [f32; 3],
}

// See <https://github.com/mozilla/pdf.js/blob/06f44916c8936b92f464d337fe3a0a6b2b78d5b4/src/core/colorspace.js#L846>
// Completely copied from there without really understanding the logic, but we get the same results as Firefox
// which should be good enough (and by viewing the `calrgb.pdf` test file in different viewers you will
// see that in many cases each viewer does whatever it wants, even Acrobat), so this is good enough for us.
impl CalRgb {
    fn new(dict: &Dict<'_>) -> Option<Self> {
        let white_point = dict.get::<[f32; 3]>(WHITE_POINT).unwrap_or([1.0, 1.0, 1.0]);
        let black_point = dict.get::<[f32; 3]>(BLACK_POINT).unwrap_or([0.0, 0.0, 0.0]);
        let matrix = dict
            .get::<[f32; 9]>(MATRIX)
            .unwrap_or([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        let gamma = dict.get::<[f32; 3]>(GAMMA).unwrap_or([1.0, 1.0, 1.0]);

        Some(Self {
            white_point,
            black_point,
            matrix,
            gamma,
        })
    }

    const BRADFORD_SCALE_MATRIX: [f32; 9] = [
        0.8951, 0.2664, -0.1614, -0.7502, 1.7135, 0.0367, 0.0389, -0.0685, 1.0296,
    ];

    const BRADFORD_SCALE_INVERSE_MATRIX: [f32; 9] = [
        0.9869929, -0.1470543, 0.1599627, 0.4323053, 0.5183603, 0.0492912, -0.0085287, 0.0400428,
        0.9684867,
    ];

    const SRGB_D65_XYZ_TO_RGB_MATRIX: [f32; 9] = [
        3.2404542, -1.5371385, -0.4985314, -0.969_266, 1.8760108, 0.0415560, 0.0556434, -0.2040259,
        1.0572252,
    ];

    const FLAT_WHITEPOINT: [f32; 3] = [1.0, 1.0, 1.0];
    const D65_WHITEPOINT: [f32; 3] = [0.95047, 1.0, 1.08883];

    fn decode_l_constant() -> f32 {
        ((8.0_f32 + 16.0) / 116.0).powi(3) / 8.0
    }

    fn srgb_transfer_function(color: f32) -> f32 {
        if color <= 0.0031308 {
            (12.92 * color).clamp(0.0, 1.0)
        } else if color >= 0.99554525 {
            1.0
        } else {
            ((1.0 + 0.055) * color.powf(1.0 / 2.4) - 0.055).clamp(0.0, 1.0)
        }
    }

    fn matrix_product(a: &[f32; 9], b: &[f32; 3]) -> [f32; 3] {
        [
            a[0] * b[0] + a[1] * b[1] + a[2] * b[2],
            a[3] * b[0] + a[4] * b[1] + a[5] * b[2],
            a[6] * b[0] + a[7] * b[1] + a[8] * b[2],
        ]
    }

    fn to_flat(source_white_point: &[f32; 3], lms: &[f32; 3]) -> [f32; 3] {
        [
            lms[0] / source_white_point[0],
            lms[1] / source_white_point[1],
            lms[2] / source_white_point[2],
        ]
    }

    fn to_d65(source_white_point: &[f32; 3], lms: &[f32; 3]) -> [f32; 3] {
        [
            lms[0] * Self::D65_WHITEPOINT[0] / source_white_point[0],
            lms[1] * Self::D65_WHITEPOINT[1] / source_white_point[1],
            lms[2] * Self::D65_WHITEPOINT[2] / source_white_point[2],
        ]
    }

    fn decode_l(l: f32) -> f32 {
        if l < 0.0 {
            -Self::decode_l(-l)
        } else if l > 8.0 {
            ((l + 16.0) / 116.0).powi(3)
        } else {
            l * Self::decode_l_constant()
        }
    }

    fn compensate_black_point(source_bp: &[f32; 3], xyz_flat: &[f32; 3]) -> [f32; 3] {
        if source_bp == &[0.0, 0.0, 0.0] {
            return *xyz_flat;
        }

        let zero_decode_l = Self::decode_l(0.0);

        let mut out = [0.0; 3];
        for i in 0..3 {
            let src = Self::decode_l(source_bp[i]);
            let scale = (1.0 - zero_decode_l) / (1.0 - src);
            let offset = 1.0 - scale;
            out[i] = xyz_flat[i] * scale + offset;
        }

        out
    }

    fn normalize_white_point_to_flat(
        &self,
        source_white_point: &[f32; 3],
        xyz: &[f32; 3],
    ) -> [f32; 3] {
        if source_white_point[0] == 1.0 && source_white_point[2] == 1.0 {
            return *xyz;
        }
        let lms = Self::matrix_product(&Self::BRADFORD_SCALE_MATRIX, xyz);
        let lms_flat = Self::to_flat(source_white_point, &lms);
        Self::matrix_product(&Self::BRADFORD_SCALE_INVERSE_MATRIX, &lms_flat)
    }

    fn normalize_white_point_to_d65(
        &self,
        source_white_point: &[f32; 3],
        xyz: &[f32; 3],
    ) -> [f32; 3] {
        let lms = Self::matrix_product(&Self::BRADFORD_SCALE_MATRIX, xyz);
        let lms_d65 = Self::to_d65(source_white_point, &lms);
        Self::matrix_product(&Self::BRADFORD_SCALE_INVERSE_MATRIX, &lms_d65)
    }
}

impl CalRgb {
    fn rgb_float(&self, input: &[f32]) -> Option<[f32; 3]> {
        if input.len() != 3
            || self.white_point.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || self.black_point.iter().chain(self.matrix.iter()).any(|v| !v.is_finite())
            || self.gamma.iter().any(|v| !v.is_finite() || *v <= 0.0)
        { return None; }
        let input = [
            input[0].clamp(0.0, 1.0),
            input[1].clamp(0.0, 1.0),
            input[2].clamp(0.0, 1.0),
        ];

        let [r, g, b] = input;
        let [gr, gg, gb] = self.gamma;
        let [agr, bgg, cgb] = [
            if r == 1.0 { 1.0 } else { r.powf(gr) },
            if g == 1.0 { 1.0 } else { g.powf(gg) },
            if b == 1.0 { 1.0 } else { b.powf(gb) },
        ];

        let m = &self.matrix;
        let x = m[0] * agr + m[3] * bgg + m[6] * cgb;
        let y = m[1] * agr + m[4] * bgg + m[7] * cgb;
        let z = m[2] * agr + m[5] * bgg + m[8] * cgb;
        let xyz = [x, y, z];

        let xyz_flat = self.normalize_white_point_to_flat(&self.white_point, &xyz);
        let xyz_black = Self::compensate_black_point(&self.black_point, &xyz_flat);
        let xyz_d65 = self.normalize_white_point_to_d65(&Self::FLAT_WHITEPOINT, &xyz_black);
        let srgb_xyz = Self::matrix_product(&Self::SRGB_D65_XYZ_TO_RGB_MATRIX, &xyz_d65);

        if srgb_xyz.iter().any(|v| !v.is_finite()) { return None; }
        Some(srgb_xyz.map(Self::srgb_transfer_function))
    }
}

impl ToRgb for CalRgb {
    fn convert_f32(&self, input: &[f32], output: &mut [u8], _: bool) -> Option<()> {
        for (input, output) in input.chunks_exact(3).zip(output.chunks_exact_mut(3)) {
            let rgb = self.rgb_float(input)?;
            output.copy_from_slice(&rgb.map(|value| (value * 255.0 + 0.5) as u8));
        }

        Some(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Lab {
    range: [f32; 4],
    profile: ICCProfile,
}

impl Lab {
    fn new(dict: &Dict<'_>) -> Option<Self> {
        let white_point = dict.get::<[f32; 3]>(WHITE_POINT).unwrap_or([1.0, 1.0, 1.0]);
        // Not sure how this should be used.
        let _black_point = dict.get::<[f32; 3]>(BLACK_POINT).unwrap_or([0.0, 0.0, 0.0]);
        let range = dict
            .get::<[f32; 4]>(RANGE)
            .unwrap_or([-100.0, 100.0, -100.0, 100.0]);

        let mut profile = ColorProfile::new_from_slice(include_bytes!("../assets/LAB.icc")).ok()?;
        profile.white_point = Xyzd::new(
            white_point[0] as f64,
            white_point[1] as f64,
            white_point[2] as f64,
        );

        let profile = ICCProfile::new_from_src_profile(
            profile, false,
            // This flag is only used to scale the values to [0.0, 1.0], but
            // we already take care of this in the `convert_f32` method.
            // Therefore, leave this as false, even though this is a LAB profile.
            false, 3,
        )?;

        Some(Self { range, profile })
    }
}

impl ToRgb for Lab {
    fn convert_f32(&self, input: &[f32], output: &mut [u8], manual_scale: bool) -> Option<()> {
        if !manual_scale {
            // moxcms expects values between 0.0 and 1.0, so we need to undo
            // the scaling.

            let input = input
                .chunks_exact(3)
                .flat_map(|i| {
                    let l = i[0] / 100.0;
                    let a = (i[1] + 128.0) / 255.0;
                    let b = (i[2] + 128.0) / 255.0;

                    [l, a, b]
                })
                .collect::<Vec<_>>();

            self.profile.convert_f32(&input, output, manual_scale)
        } else {
            self.profile.convert_f32(input, output, manual_scale)
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Indexed {
    values: Vec<Vec<f32>>,
    hival: u8,
    base: Box<ColorSpace>,
}

impl Indexed {
    fn new(array: &Array<'_>, cache: &Cache) -> Option<Self> {
        let mut iter = array.flex_iter();
        // Skip name
        let _ = iter.next::<Name<'_>>()?;
        let base_color_space = ColorSpace::new(iter.next::<Object<'_>>()?, cache)?;
        let hival = iter.next::<u32>()?.min(u8::MAX as u32) as u8;

        let values = {
            let data = iter
                .next::<Stream<'_>>()
                .and_then(|s| s.decoded().ok())
                .or_else(|| {
                    iter.next::<object::String<'_>>()
                        .map(|s| Cow::Owned(s.to_vec()))
                })?;

            let num_components = base_color_space.num_components();

            let mut byte_iter = data.iter().copied();

            let mut vals = vec![];
            for _ in 0..=hival {
                let mut temp = vec![];

                for _ in 0..num_components {
                    temp.push(byte_iter.next()? as f32 / 255.0);
                }

                vals.push(temp);
            }

            vals
        };

        Some(Self {
            values,
            hival,
            base: Box::new(base_color_space),
        })
    }
}

impl ToRgb for Indexed {
    fn convert_f32(&self, input: &[f32], output: &mut [u8], _: bool) -> Option<()> {
        let mut indexed = vec![0.0; input.len() * self.base.num_components() as usize];

        for (input, output) in input
            .iter()
            .copied()
            .zip(indexed.chunks_exact_mut(self.base.num_components() as usize))
        {
            let idx = (input.clamp(0.0, self.hival as f32) + 0.5) as usize;
            output.copy_from_slice(&self.values[idx]);
        }

        self.base.convert_f32(&indexed, output, true)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Separation {
    alternate_space: ColorSpace,
    tint_transform: Function,
    is_none_separation: bool,
}

impl Separation {
    fn new(array: &Array<'_>, cache: &Cache) -> Option<Self> {
        let mut iter = array.flex_iter();
        // Skip `/Separation`
        let _ = iter.next::<Name<'_>>()?;
        let name = iter.next::<Name<'_>>()?;
        let alternate_space = ColorSpace::new(iter.next::<Object<'_>>()?, cache)?;
        let tint_transform = Function::new(&iter.next::<Object<'_>>()?)?;
        // Either I did something wrong, or no other viewers properly handles
        // `All`, so let's just ignore it as well.
        let is_none_separation = name.as_str() == "None";

        Some(Self {
            alternate_space,
            tint_transform,
            is_none_separation,
        })
    }
}

impl ToRgb for Separation {
    fn convert_f32(&self, input: &[f32], output: &mut [u8], _: bool) -> Option<()> {
        let evaluated = input
            .iter()
            .flat_map(|n| {
                self.tint_transform
                    .eval(smallvec![*n])
                    .unwrap_or(self.alternate_space.initial_color())
            })
            .collect::<Vec<_>>();
        self.alternate_space.convert_f32(&evaluated, output, false)
    }

    fn is_none(&self) -> bool {
        self.is_none_separation
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DeviceN {
    alternate_space: ColorSpace,
    num_components: u8,
    tint_transform: Function,
    is_none: bool,
}

impl DeviceN {
    fn new(array: &Array<'_>, cache: &Cache) -> Option<Self> {
        let mut iter = array.flex_iter();
        // Skip `/DeviceN`
        let _ = iter.next::<Name<'_>>()?;
        // Skip `Name`.
        let names = iter
            .next::<Array<'_>>()?
            .iter::<Name<'_>>()
            .collect::<Vec<_>>();
        let num_components = u8::try_from(names.len()).ok()?;
        let all_none = names.iter().all(|n| n.as_str() == "None");
        let alternate_space = ColorSpace::new(iter.next::<Object<'_>>()?, cache)?;
        let tint_transform = Function::new(&iter.next::<Object<'_>>()?)?;

        if num_components == 0 {
            return None;
        }

        Some(Self {
            alternate_space,
            num_components,
            tint_transform,
            is_none: all_none,
        })
    }
}

impl ToRgb for DeviceN {
    fn convert_f32(&self, input: &[f32], output: &mut [u8], _: bool) -> Option<()> {
        let evaluated = input
            .chunks_exact(self.num_components as usize)
            .flat_map(|n| {
                self.tint_transform
                    .eval(n.to_smallvec())
                    .unwrap_or(self.alternate_space.initial_color())
            })
            .collect::<Vec<_>>();
        self.alternate_space.convert_f32(&evaluated, output, false)
    }

    fn is_none(&self) -> bool {
        self.is_none
    }
}

struct ICCColorRepr {
    src_profile: ColorProfile,
    src_layout: Layout,
    number_components: usize,
    is_srgb: bool,
    is_lab: bool,
    transform_u8: Arc<Transform8BitExecutor>,
    transform_f32: OnceLock<Arc<TransformF32Executor>>,
}

#[derive(Clone)]
pub(crate) struct ICCProfile(Arc<ICCColorRepr>);

impl Debug for ICCProfile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ICCColor {{..}}")
    }
}

impl ICCProfile {
    fn new(profile: &[u8], number_components: usize) -> Option<Self> {
        let src_profile = ColorProfile::new_from_slice(profile).ok()?;

        const SRGB_MARKER: &[u8] = b"sRGB";

        let is_srgb = profile
            .get(52..56)
            .map(|device_model| device_model == SRGB_MARKER)
            .unwrap_or(false);
        let is_lab = src_profile.color_space == DataColorSpace::Lab;

        Self::new_from_src_profile(src_profile, is_srgb, is_lab, number_components)
    }

    fn new_from_src_profile(
        src_profile: ColorProfile,
        is_srgb: bool,
        is_lab: bool,
        number_components: usize,
    ) -> Option<Self> {
        let src_layout = match number_components {
            1 => Layout::Gray,
            3 => Layout::Rgb,
            4 => Layout::Rgba,
            _ => {
                warn!("unsupported number of components {number_components} for ICC profile");

                return None;
            }
        };

        let dest_profile = ColorProfile::new_srgb();
        let transform_u8 = src_profile
            .clone()
            .create_transform_8bit(
                src_layout,
                &dest_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .ok()?;

        Some(Self(Arc::new(ICCColorRepr {
            src_profile,
            src_layout,
            number_components,
            is_srgb,
            is_lab,
            transform_u8,
            transform_f32: OnceLock::new(),
        })))
    }

    fn is_srgb(&self) -> bool {
        self.0.is_srgb
    }

    fn is_lab(&self) -> bool {
        self.0.is_lab
    }

    fn transform_u8(&self) -> &Arc<Transform8BitExecutor> {
        &self.0.transform_u8
    }

    fn transform_f32(&self) -> &Arc<TransformF32Executor> {
        // From my benchmarking, creating the f32 transforms is usually much
        // more expensive than u8. Therefore, we only create it lazily when
        // really needed.
        self.0.transform_f32.get_or_init(|| {
            let dest_profile = ColorProfile::new_srgb();
            self.0
                .src_profile
                .clone()
                .create_transform_f32(
                    self.0.src_layout,
                    &dest_profile,
                    Layout::Rgb,
                    TransformOptions::default(),
                )
                // Since the u8 version was valid, hopefully this should never panic?
                .unwrap()
        })
    }
}

impl ToRgb for ICCProfile {
    fn convert_f32(&self, input: &[f32], output: &mut [u8], _: bool) -> Option<()> {
        let mut temp = vec![0.0_f32; output.len()];

        if self.is_lab() {
            // moxcms expects normalized values.
            let scaled = input
                .chunks_exact(3)
                .flat_map(|i| {
                    [
                        i[0] * (1.0 / 100.0),
                        (i[1] + 128.0) * (1.0 / 255.0),
                        (i[2] + 128.0) * (1.0 / 255.0),
                    ]
                })
                .collect::<Vec<_>>();
            self.transform_f32().transform(&scaled, &mut temp).ok()?;
        } else {
            self.transform_f32().transform(input, &mut temp).ok()?;
        };

        for (input, output) in temp.iter().zip(output.iter_mut()) {
            *output = (input * 255.0 + 0.5) as u8;
        }

        Some(())
    }

    fn supports_u8(&self) -> bool {
        true
    }

    fn convert_u8(&self, input: &[u8], output: &mut [u8]) -> Option<()> {
        if self.is_srgb() {
            output.copy_from_slice(input);
        } else {
            self.transform_u8().transform(input, output).ok()?;
        }

        Some(())
    }
}

#[inline(always)]
fn f32_to_u8(val: f32) -> u8 {
    (val * 255.0 + 0.5) as u8
}

#[derive(Debug, Clone)]
/// A color.
pub struct Color {
    color_space: ColorSpace,
    components: ColorComponents,
    opacity: f32,
    conversion_functions: DeviceConversionFunctions,
}

/// A graphics-state black-generation or undercolor-removal function.
#[derive(Debug, Clone, Default)]
pub enum DeviceConversionFunction {
    /// Device default; a concrete output policy must resolve this later.
    #[default]
    Default,
    /// Explicit PDF function.
    Function(Function),
    /// Declared but invalid; native conversion must refuse it.
    Unresolved,
}

/// Original graphics-state functions used for RGB-to-CMYK conversion.
#[derive(Debug, Clone, Default)]
pub struct DeviceConversionFunctions {
    /// Function selecting generated black.
    pub black_generation: DeviceConversionFunction,
    /// Function selecting removed undercolor.
    pub undercolor_removal: DeviceConversionFunction,
}

impl Color {
    /// Original device conversion functions, before output-policy resolution.
    pub fn conversion_functions(&self) -> &DeviceConversionFunctions {
        &self.conversion_functions
    }

    pub(crate) fn with_conversion_functions(mut self, functions: DeviceConversionFunctions) -> Self {
        self.conversion_functions = functions;
        self
    }
    /// Original PDF color space, retained for transparency-group blending.
    pub fn color_space(&self) -> &ColorSpace {
        &self.color_space
    }

    /// Native PDF components, without display conversion or 8-bit quantization.
    pub fn components(&self) -> &[f32] {
        &self.components
    }

    /// Paint opacity, independent of native color components and geometric shape.
    pub fn opacity(&self) -> f32 {
        self.opacity
    }

    pub(crate) fn new(color_space: ColorSpace, components: ColorComponents, opacity: f32) -> Self {
        Self {
            color_space,
            components,
            opacity,
            conversion_functions: DeviceConversionFunctions::default(),
        }
    }

    /// Return the color as an RGBA color.
    pub fn to_rgba(&self) -> AlphaColor {
        self.color_space
            .to_rgba(&self.components, self.opacity, false)
    }

    /// Create a color from RGBA.
    pub fn from_rgba(rgba: AlphaColor) -> Self {
        let c = rgba.components();
        Self {
            color_space: ColorSpace::device_rgb(),
            components: smallvec![c[0], c[1], c[2]],
            opacity: c[3],
            conversion_functions: DeviceConversionFunctions::default(),
        }
    }
}

static CMYK_TRANSFORM: LazyLock<ICCProfile> = LazyLock::new(|| {
    ICCProfile::new(include_bytes!("../assets/CGATS001Compat-v2-micro.icc"), 4).unwrap()
});

static DEVICE_CMYK_FLOAT: LazyLock<Arc<TransformF32Executor>> = LazyLock::new(|| {
    CMYK_TRANSFORM.0.src_profile.create_transform_f32(Layout::Rgba, &ColorProfile::new_srgb(), Layout::Rgb,
        TransformOptions { rrrah_cmyk_hybrid: true, ..TransformOptions::default() }).unwrap()
});

pub(crate) trait ToRgb {
    fn prefers_float_samples(&self) -> bool { false }
    fn convert_sample(&self, input: &[f32], output: &mut [u8], manual_scale: bool) -> Option<()> {
        // We prefer using the u8 variant for single samples, which is especially
        // important for ICC profiles to avoid constructing the (more expensive)
        // f32 variant.
        if !self.prefers_float_samples() && self.supports_u8() {
            let converted = input
                .iter()
                .copied()
                .map(f32_to_u8)
                .collect::<SmallVec<[u8; 4]>>();

            if self.convert_u8(&converted, output).is_some() {
                return Some(());
            }
        }

        self.convert_f32(input, output, manual_scale)
    }

    fn convert_f32(&self, input: &[f32], output: &mut [u8], manual_scale: bool) -> Option<()>;
    fn supports_u8(&self) -> bool {
        false
    }
    fn convert_u8(&self, _: &[u8], _: &mut [u8]) -> Option<()> {
        unimplemented!();
    }
    fn is_none(&self) -> bool {
        false
    }
    fn to_alpha_color(
        &self,
        input: &[f32],
        mut opacity: f32,
        manual_scale: bool,
    ) -> Option<AlphaColor> {
        let mut output = [0; 3];
        self.convert_sample(input, &mut output, manual_scale)?;

        // For separation color spaces:
        // "The special colourant name None shall not produce any visible output.
        // Painting operations in a Separation space with this colourant name
        // shall have no effect on the current page."
        if self.is_none() {
            opacity = 0.0;
        }

        Some(AlphaColor::from_rgba8(
            output[0],
            output[1],
            output[2],
            (opacity * 255.0 + 0.5) as u8,
        ))
    }
}

/// Device replacements are resolved from the current lexical resource scope.
#[derive(Clone, Default)]
pub(crate) struct DeviceColorDefaults {
    spaces: [Option<ColorSpace>; 3],
}
impl DeviceColorDefaults {
    pub(crate) fn new(resources: &hayro_syntax::page::Resources<'_>, cache: &Cache) -> Self {
        let names: [&[u8]; 3] = [b"DefaultGray", b"DefaultRGB", b"DefaultCMYK"];
        let components = [1, 3, 4];
        Self { spaces: std::array::from_fn(|i| {
            let object = resources.get_color_space(&Name::new_unescaped(names[i]))?;
            let cs: ColorSpace = cache.get_or_insert_with(object.cache_key(), || ColorSpace::new(object.clone(), cache))?;
            let calibrated = matches!(cs.0.as_ref(), ColorSpaceType::ICCBased(_) | ColorSpaceType::CalGray(_) | ColorSpaceType::CalRgb(_) | ColorSpaceType::Lab(_));
            (calibrated && cs.num_components() == components[i]).then_some(cs)
        }) }
    }
    pub(crate) fn cache_key(&self) -> u128 {
        if self.spaces.iter().all(Option::is_none) { return 0; }
        crate::util::hash128(&self.spaces.each_ref().map(|cs| cs.as_ref().map(|cs| cs.1)))
    }
}
impl ColorSpace {
    pub(crate) fn with_device_defaults(&self, defaults: &DeviceColorDefaults) -> Self {
        if defaults.spaces.iter().all(Option::is_none) { return self.clone(); }
        let replacement = match self.0.as_ref() {
            ColorSpaceType::DeviceGray => return defaults.spaces[0].clone().unwrap_or_else(|| self.clone()),
            ColorSpaceType::DeviceRgb => return defaults.spaces[1].clone().unwrap_or_else(|| self.clone()),
            ColorSpaceType::DeviceCmyk => return defaults.spaces[2].clone().unwrap_or_else(|| self.clone()),
            ColorSpaceType::Pattern(base) => ColorSpaceType::Pattern(base.with_device_defaults(defaults)),
            ColorSpaceType::Indexed(indexed) => { let mut value = indexed.clone(); value.base = Box::new(value.base.with_device_defaults(defaults)); ColorSpaceType::Indexed(value) },
            ColorSpaceType::Separation(separation) => { let mut value = separation.clone(); value.alternate_space = value.alternate_space.with_device_defaults(defaults); ColorSpaceType::Separation(value) },
            ColorSpaceType::DeviceN(device) => { let mut value = device.clone(); value.alternate_space = value.alternate_space.with_device_defaults(defaults); ColorSpaceType::DeviceN(value) },
            _ => return self.clone(),
        };
        Self(Arc::new(replacement), crate::util::hash128(&(self.1, defaults.cache_key())))
    }
}

#[cfg(test)]
mod native_color_access_tests {
    use super::*;

    #[test]
    fn preserves_native_components_and_opacity_across_display_conversion() {
        for (space, components) in [
            (ColorSpace::device_cmyk(), smallvec![0.17, 0.31, 0.53, 0.07]),
            (ColorSpace::device_rgb(), smallvec![0.17, 0.31, 0.53]),
            (ColorSpace::device_gray(), smallvec![0.17]),
        ] {
            let color = Color::new(space, components.clone(), 0.37);
            assert_eq!(color.color_space().num_components() as usize, components.len());
            assert_eq!(color.components(), components.as_slice());
            assert_eq!(color.opacity(), 0.37);
            // Display conversion currently quantizes opacity as well as color.
            // Native access must retain the original samples regardless.
            let _display = color.to_rgba();
            assert_eq!(color.components(), components.as_slice());
            assert_eq!(color.opacity(), 0.37);
        }
    }
}

#[cfg(test)]
mod blending_coordinate_tests {
    use super::*;
    #[test]
    fn explicit_bg_ucr_cmyk_conversion_is_bounded_and_cancellable() {
        use hayro_syntax::object::{Dict, FromBytes};
        let function = |bytes: &[u8]| DeviceConversionFunction::Function(
            Function::new(&Object::Dict(Dict::from_bytes(bytes).unwrap())).unwrap()
        );
        let functions = DeviceConversionFunctions {
            black_generation: function(b"<< /FunctionType 2 /Domain [0 1] /C0 [0.1] /C1 [0.3] /N 1 >>"),
            undercolor_removal: function(b"<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >>"),
        };
        let rgb = ColorSpace::device_rgb();
        let cmyk = ColorSpace::device_cmyk();
        let input = [0.2,0.4,0.6];
        let result = rgb.rgb_sample_to_cmyk_with(&cmyk, &input, &functions, &|| false).unwrap();
        for (actual, expected) in result.into_iter().zip([0.4,0.2,0.0,0.18]) {
            assert!((actual-expected).abs() < 1e-7);
        }
        for stop in 1..=6 {
            let polls = std::cell::Cell::new(0);
            assert!(rgb.rgb_sample_to_cmyk_with(&cmyk, &input, &functions, &|| {
                polls.set(polls.get()+1); polls.get() == stop
            }).is_none());
        }
        assert!(rgb.rgb_sample_to_cmyk_with(&cmyk, &input, &DeviceConversionFunctions::default(), &|| false).is_none());
        let mut invalid = functions.clone();
        invalid.black_generation = function(b"<< /FunctionType 2 /Domain [0 1] /C0 [0 0] /C1 [1 1] /N 1 >>");
        assert!(rgb.rgb_sample_to_cmyk_with(&cmyk, &input, &invalid, &|| false).is_none());
    }
    #[test]
    fn device_gray_cmyk_boundaries_follow_independent_references() {
        let gray = ColorSpace::device_gray();
        let rgb = ColorSpace::device_rgb();
        let cmyk = ColorSpace::device_cmyk();
        for (input, expected) in [([1.,0.,0.],0.3),([0.,1.,0.],0.59),([0.,0.,1.],0.11),([0.2,0.4,0.6],0.362)] {
            assert!((rgb.device_sample_to_gray_or_cmyk(&gray, &input).unwrap()[0] - expected).abs() < 1e-7);
        }
        assert!((cmyk.device_sample_to_gray_or_cmyk(&gray, &[0.2,0.4,0.6,0.1]).unwrap()[0] - 0.538).abs() < 1e-7);
        assert_eq!(cmyk.device_sample_to_gray_or_cmyk(&gray, &[1.;4]).unwrap()[0],0.0);
        assert_eq!(gray.device_sample_to_gray_or_cmyk(&cmyk, &[0.25]).unwrap(),[0.,0.,0.,0.75]);
        assert!(rgb.device_sample_to_gray_or_cmyk(&cmyk, &[0.2,0.4,0.6]).is_none());
        assert!(rgb.device_sample_to_gray_or_cmyk(&gray, &[f32::NAN;3]).is_none());
    }
    #[test]
    fn calibrated_float_policy_retains_fractions_and_refuses_invalid_parameters() {
        let rgb = ColorSpace::device_rgb();
        let gray = ColorSpace(Arc::new(ColorSpaceType::CalGray(CalGray {
            white_point: [1.0; 3], black_point: [0.0; 3], gamma: 1.0,
        })), 0);
        // Cube-root display policy has the independent reference 107.1/255
        // at luminance1/8. This is a policy control, not a PDF conformance oracle.
        let sample = gray.calibrated_sample_to_rgb(&rgb, &[0.125]).unwrap();
        assert!((sample[0] - 107.1 / 255.0).abs() < 0.000001);
        assert_eq!(sample[..3], [sample[0]; 3]);
        assert_eq!(gray.calibrated_sample_to_rgb(&rgb, &[0.0]).unwrap()[..3], [0.0; 3]);
        let calibrated = |gamma| ColorSpace(Arc::new(ColorSpaceType::CalRgb(CalRgb {
            white_point: [1.0; 3], black_point: [0.0; 3],
            matrix: [1.,0.,0.,0.,1.,0.,0.,0.,1.], gamma: [gamma; 3],
        })), 0);
        let cal = calibrated(1.0);
        assert_eq!(cal.calibrated_sample_to_rgb(&rgb, &[0.0; 3]).unwrap()[..3], [0.0; 3]);
        let input = [0.173123, 0.319876, 0.537654];
        let converted = cal.calibrated_sample_to_rgb(&rgb, &input).unwrap();
        assert!(converted[..3].iter().any(|v| (v*255.0 - (v*255.0).round()).abs() > 0.01));
        let darker = calibrated(2.0).calibrated_sample_to_rgb(&rgb, &input).unwrap();
        assert!(darker[1] < converted[1]);
        assert!(calibrated(f32::NAN).calibrated_sample_to_rgb(&rgb, &input).is_none());
        assert!(cal.calibrated_sample_to_rgb(&rgb, &[f32::NAN;3]).is_none());
        assert!(cal.calibrated_sample_to_rgb(&gray, &input).is_none());
    }
    #[test]
    fn device_rgb_policy_avoids_display_quantization() {
        let rgb = ColorSpace::device_rgb();
        let sample = [0.173123, 0.319876, 0.537654];
        assert_eq!(rgb.device_sample_to_rgb(&rgb, &sample).unwrap()[..3], sample);
        let gray = ColorSpace::device_gray();
        assert_eq!(gray.device_sample_to_rgb(&rgb, &[0.371234]).unwrap()[..3], [0.371234; 3]);
        let cmyk = ColorSpace::device_cmyk();
        let input = [0.173123, 0.319876, 0.537654, 0.071234];
        let converted = cmyk.device_sample_to_rgb(&rgb, &input).unwrap();
        let display = cmyk.to_rgba(&input, 1.0, false).to_rgba8();
        for i in 0..3 { assert_eq!(f32_to_u8(converted[i]), display[i]); }
        assert!(converted[..3].iter().any(|v| (v * 255.0 - (v * 255.0).round()).abs() > 0.01));
        assert!(gray.device_sample_to_rgb(&cmyk, &[0.5]).is_none());
        assert!(rgb.device_sample_to_rgb(&rgb, &[f32::NAN, 0.0, 0.0]).is_none());
        assert!(rgb.device_sample_to_rgb(&rgb, &[1.1, 0.0, 0.0]).is_none());
    }
    #[test]
    fn explicit_icc_blending_transform_preserves_float_samples() {
        let space = || ColorSpace(Arc::new(ColorSpaceType::ICCBased(
            ICCProfile::new_from_src_profile(ColorProfile::new_srgb(), true, false, 3).unwrap()
        )), 0);
        let source = space();
        let destination = space();
        assert!(!source.shares_blending_coordinates(&destination));
        let transform = source.blending_transform_to(&destination).unwrap();
        let input = [0.173123, 0.319876, 0.537654];
        let output = transform.convert(&input).unwrap();
        for i in 0..3 {
            // ICC matrix round trips are approximate; this is a sub-8-bit
            // precision control, not an exact profile-equivalence assertion.
            assert!((input[i] - output[i]).abs() < 0.5 / 255.0, "input={input:?} output={output:?}");
            assert!((output[i] * 255.0 - (output[i] * 255.0).round()).abs() > 0.01);
        }
        assert!(transform.convert(&[0.5]).is_none());
        assert!(transform.convert(&[f32::NAN, 0.0, 0.0]).is_none());
        assert!(transform.convert(&[1.01, 0.0, 0.0]).is_none());
        assert!(source.blending_transform_to(&ColorSpace::device_rgb()).is_none());
        let gray = |gamma| ColorSpace(Arc::new(ColorSpaceType::ICCBased(
            ICCProfile::new_from_src_profile(ColorProfile::new_gray_with_gamma(gamma), false, false, 1).unwrap()
        )), 0);
        let gamma_transform = gray(1.0).blending_transform_to(&gray(2.0)).unwrap();
        let converted = gamma_transform.convert(&[0.25]).unwrap();
        assert!((converted[0] - 0.5).abs() < 0.0002, "{converted:?}");
    }
    #[test]
    fn device_calibration_and_shared_profile_coordinates_are_distinct() {
        assert!(ColorSpace::device_rgb().shares_blending_coordinates(&ColorSpace::device_rgb()));
        assert!(!ColorSpace::device_rgb().shares_blending_coordinates(&ColorSpace::device_cmyk()));
        let calibrated = |gamma| ColorSpace(Arc::new(ColorSpaceType::CalRgb(CalRgb {
            white_point:[0.9505,1.,1.089], black_point:[0.;3],
            matrix:[1.,0.,0.,0.,1.,0.,0.,0.,1.], gamma:[gamma;3],
        })),0);
        let a=calibrated(1.);
        assert!(a.shares_blending_coordinates(&calibrated(1.)));
        assert!(!a.shares_blending_coordinates(&calibrated(2.2)));
        assert!(!a.shares_blending_coordinates(&ColorSpace::device_rgb()));
        let profile=CMYK_TRANSFORM.clone();
        let p=ColorSpace(Arc::new(ColorSpaceType::ICCBased(profile.clone())),0);
        let q=ColorSpace(Arc::new(ColorSpaceType::ICCBased(profile)),0);
        assert_eq!(p.blending_model(),Some(BlendingModel::Cmyk));
        assert!(p.shares_blending_coordinates(&q));
        assert!(!p.shares_blending_coordinates(&ColorSpace::device_cmyk()));
        let separate=ColorSpace(Arc::new(ColorSpaceType::ICCBased(ICCProfile::new(include_bytes!("../assets/CGATS001Compat-v2-micro.icc"),4).unwrap())),0);
        assert!(!p.shares_blending_coordinates(&separate));
        assert!(!ColorSpace::pattern().shares_blending_coordinates(&ColorSpace::pattern()));
    }
}
