//! Interleaved, straight-alpha raster data, separate from sensor mosaics.

use rrrah_memory::PixelBuffer;
use std::sync::Arc;

use thiserror::Error;

/// Original sample precision is retained, including HDR values above one.
#[derive(Debug, Clone)]
pub enum RasterPixels {
    Rgba8(PixelBuffer<u8>),
    Rgba16(PixelBuffer<u16>),
    Rgba32Float(PixelBuffer<f32>),
}

impl RasterPixels {
    /// Allocated sample capacity, including spare vector capacity.
    /// Metadata, allocator bookkeeping and GPU textures are excluded.
    pub fn capacity_bytes(&self) -> u64 {
        match self {
            Self::Rgba8(values) => values.capacity_bytes(),
            Self::Rgba16(values) => values.capacity_bytes(),
            Self::Rgba32Float(values) => values.capacity_bytes(),
        }
    }

    pub fn try_manage(self, budget: &rrrah_memory::MemoryBudget) -> Result<Self, rrrah_memory::BufferError> {
        Ok(match self {
            Self::Rgba8(values) => Self::Rgba8(values.try_manage(budget)?),
            Self::Rgba16(values) => Self::Rgba16(values.try_manage(budget)?),
            Self::Rgba32Float(values) => Self::Rgba32Float(values.try_manage(budget)?),
        })
    }

    fn len(&self) -> usize {
        match self {
            Self::Rgba8(p) => p.len(),
            Self::Rgba16(p) => p.len(),
            Self::Rgba32Float(p) => p.len(),
        }
    }
}

/// Interpretation of RGB samples. Embedded profiles need color management
/// before display; their samples must not be silently treated as sRGB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RasterColorSpace {
    Srgb,
    /// Untagged image imported with the conventional sRGB fallback.
    AssumedSrgb,
    /// Full-range BT.709 transfer, D65/BT.709 primaries (standard PPM/PGM).
    Bt709,
    LinearSrgb,
    /// Linear samples with unqualified primaries/white point (e.g. Radiance).
    LinearRgbUnspecified,
    Icc(Vec<u8>),
    /// Container CICP declaration. Requires a qualified display transform;
    /// retain HDR and unfamiliar declarations rather than erase them.
    Cicp {
        primaries: u16,
        transfer: u16,
        matrix: u16,
        full_range: bool,
    },
    Unspecified,
}

#[derive(Debug, Clone)]
pub struct DecodedRaster {
    width: u32,
    height: u32,
    pixels: RasterPixels,
    color_space: Arc<RasterColorMetadata>,
    sample_scale: f32,
    hotspot: Option<(u32, u32)>,
    image_index: usize,
    image_count: usize,
}

#[derive(Debug)]
struct RasterColorMetadata {
    space: RasterColorSpace,
    reservation: Option<rrrah_memory::Reservation>,
}

static SRGB8_LINEAR: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
fn srgb8_table() -> [f32; 256] {
    let mut table = [0.0; 256];
    for (sample, linear) in (0_u8..=u8::MAX).zip(&mut table) {
        *linear = srgb_linear(f32::from(sample) / 255.0);
    }
    table
}
fn srgb_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
impl DecodedRaster {
    /// Convert PQ/BT.2020 RGB to linear sRGB, with 1.0 equal to the declared
    /// reference white in cd/m². No tone mapping or gamut clipping is applied.
    pub fn pq_to_linear_srgb_with_budget_and_cancel(
        &self,
        reference_white_nits: f32,
        budget: Option<&rrrah_memory::MemoryBudget>,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, RasterError> {
        self.hdr_to_linear_srgb(reference_white_nits, None, budget, cancel)
    }

    /// HLG BT.2020 display conversion for a zero-black reference display.
    /// Peak luminance, reference white and system gamma are explicit; no clipping.
    pub fn hlg_to_linear_srgb_with_budget_and_cancel(
        &self,
        reference_white_nits: f32,
        peak_nits: f32,
        system_gamma: f32,
        budget: Option<&rrrah_memory::MemoryBudget>,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, RasterError> {
        self.hdr_to_linear_srgb(
            reference_white_nits,
            Some((peak_nits, system_gamma)),
            budget,
            cancel,
        )
    }

    fn hdr_to_linear_srgb(
        &self,
        reference_white_nits: f32,
        hlg: Option<(f32, f32)>,
        budget: Option<&rrrah_memory::MemoryBudget>,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, RasterError> {
        if cancel() {
            return Err(RasterError::Cancelled);
        }
        if !reference_white_nits.is_finite() || !(1.0..=10000.0).contains(&reference_white_nits) {
            return Err(RasterError::InvalidHdrReferenceWhite);
        }
        if let Some((peak, gamma)) = hlg {
            if !peak.is_finite()
                || !(1.0..=10000.0).contains(&peak)
                || !gamma.is_finite()
                || !(0.1..=4.0).contains(&gamma)
            {
                return Err(RasterError::InvalidHlgDisplayPolicy);
            }
        }
        let expected_transfer = if hlg.is_some() { 18 } else { 16 };
        if !matches!(self.color_space.space, RasterColorSpace::Cicp { primaries: 9, transfer, .. } if transfer == expected_transfer)
        {
            return Err(RasterError::ColorTransformRequired);
        }
        let count = self.pixels.len();
        let reservation = budget
            .map(|b| {
                b.try_reserve(
                    (count as u64)
                        .checked_mul(4)
                        .ok_or(rrrah_memory::BufferError::Overflow)?,
                )
            })
            .transpose()?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(count)
            .map_err(|_| RasterError::Allocation)?;
        for index in 0..count / 4 {
            if index.is_multiple_of(4096) && cancel() {
                return Err(RasterError::Cancelled);
            }
            let at = index * 4;
            let rgba: [f64; 4] = std::array::from_fn(|channel| match &self.pixels {
                RasterPixels::Rgba8(p) => f64::from(p[at + channel]) / 255.0,
                RasterPixels::Rgba16(p) => f64::from(p[at + channel]) / 65535.0,
                RasterPixels::Rgba32Float(p) => f64::from(p[at + channel]),
            });
            if rgba.iter().any(|v| !v.is_finite()) {
                return Err(RasterError::NonFiniteSample);
            }
            if !(0.0..=1.0).contains(&rgba[3]) {
                return Err(RasterError::InvalidAlpha);
            }
            if rgba[..3].iter().any(|v| !(0.0..=1.0).contains(v)) {
                return Err(RasterError::InvalidPqSample);
            }
            let rgb: [f64; 3] = if let Some((peak, gamma)) = hlg {
                // BT.2100 HLG inverse OETF and luminance-dependent reference OOTF.
                let scene: [f64; 3] = std::array::from_fn(|c| {
                    let value = rgba[c];
                    if value <= 0.5 {
                        value * value / 3.0
                    } else {
                        (((value - 0.55991073) / 0.17883277).exp() + 0.28466892) / 12.0
                    }
                });
                let luminance = 0.2627 * scene[0] + 0.6780 * scene[1] + 0.0593 * scene[2];
                let gain = if luminance == 0.0 {
                    0.0
                } else {
                    luminance.powf(f64::from(gamma) - 1.0) * f64::from(peak) / f64::from(reference_white_nits)
                };
                scene.map(|value| value * gain)
            } else {
                // ST 2084 inverse transfer, absolute peak luminance 10000 cd/m².
                std::array::from_fn(|c| {
                    let p = rgba[c].powf(32.0 / 2523.0);
                    let linear = ((p - 107.0 / 128.0).max(0.0) / (2413.0 / 128.0 - 2392.0 / 128.0 * p))
                        .powf(8192.0 / 1305.0);
                    linear * 10000.0 / f64::from(reference_white_nits)
                })
            };
            // BT.2020 D65 -> BT.709/sRGB D65, derived from standard xy primaries.
            for row in [
                [1.6604910021084345, -0.5876411387885495, -0.07284986331988488],
                [-0.12455047452159074, 1.1328998971259603, -0.008349422604369477],
                [-0.018150763354905303, -0.10057889800800739, 1.1187296613629127],
            ] {
                output.push((row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2]) as f32);
            }
            output.push(rgba[3] as f32);
        }
        if cancel() {
            return Err(RasterError::Cancelled);
        }
        let pixels = match reservation {
            Some(r) => r.try_adopt(output)?.into(),
            None => Arc::new(output).into(),
        };
        Self::new(
            self.width,
            self.height,
            RasterPixels::Rgba32Float(pixels),
            RasterColorSpace::LinearSrgb,
        )?
        .with_sample_scale(self.sample_scale)?
        .with_hotspot(self.hotspot)?
        .with_image_selection(self.image_index, self.image_count)
    }
    /// Mutates native samples only while this allocation has one owner.
    /// Slice length and the raster's metadata remain unchanged.
    pub fn rgba8_mut(&mut self) -> Option<&mut [u8]> {
        match &mut self.pixels {
            RasterPixels::Rgba8(p) => p.get_mut(),
            _ => None,
        }
    }
    /// Exclusive access to native sixteen-bit samples, without copying.
    pub fn rgba16_mut(&mut self) -> Option<&mut [u16]> {
        match &mut self.pixels {
            RasterPixels::Rgba16(p) => p.get_mut(),
            _ => None,
        }
    }
    /// Adopts exclusive pixel storage and admits owned ICC metadata without
    /// changing image interpretation. Managed clones share both allocations.
    pub fn try_manage_pixels(
        mut self,
        budget: &rrrah_memory::MemoryBudget,
    ) -> Result<Self, rrrah_memory::BufferError> {
        self.pixels = self.pixels.try_manage(budget)?;
        if matches!(&self.color_space.space, RasterColorSpace::Icc(_))
            && self.color_space.reservation.is_none()
        {
            let bytes = self.color_profile_capacity_bytes();
            self = self.with_color_profile_reservation(budget.try_reserve(bytes)?)?;
        }
        Ok(self)
    }

    /// Attaches admission reserved before profile allocation. Metadata clones
    /// share both the profile allocation and its reservation.
    pub fn with_color_profile_reservation(
        mut self,
        mut reservation: rrrah_memory::Reservation,
    ) -> Result<Self, rrrah_memory::BufferError> {
        reservation.ensure_bytes(self.color_profile_capacity_bytes())?;
        let metadata = Arc::get_mut(&mut self.color_space).ok_or(rrrah_memory::BufferError::SharedOwners)?;
        metadata.reservation = Some(reservation);
        Ok(self)
    }

    pub fn color_profile_capacity_bytes(&self) -> u64 {
        match &self.color_space.space {
            RasterColorSpace::Icc(profile) => profile.capacity() as u64,
            _ => 0,
        }
    }

    /// Pixel and ICC allocation capacity, excluding allocator bookkeeping.
    pub fn capacity_bytes(&self) -> u64 {
        self.pixel_capacity_bytes()
            .saturating_add(self.color_profile_capacity_bytes())
    }

    /// Pixels are row-major RGBA with straight alpha, after EXIF orientation.
    pub fn new(
        width: u32,
        height: u32,
        pixels: RasterPixels,
        color_space: RasterColorSpace,
    ) -> Result<Self, RasterError> {
        if width == 0 || height == 0 {
            return Err(RasterError::Empty);
        }
        let expected = usize::try_from(width)
            .ok()
            .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
            .and_then(|n| n.checked_mul(4))
            .ok_or(RasterError::Dimensions)?;
        if pixels.len() != expected {
            return Err(RasterError::SampleCount {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
            color_space: Arc::new(RasterColorMetadata {
                space: color_space,
                reservation: None,
            }),
            sample_scale: 1.0,
            hotspot: None,
            image_index: 0,
            image_count: 1,
        })
    }

    pub fn image_index(&self) -> usize {
        self.image_index
    }
    pub fn image_count(&self) -> usize {
        self.image_count
    }
    /// Ordinal and number of selectable images in the decoded container.
    pub fn with_image_selection(mut self, index: usize, count: usize) -> Result<Self, RasterError> {
        if count == 0 || index >= count {
            return Err(RasterError::InvalidImageSelection);
        }
        self.image_index = index;
        self.image_count = count;
        Ok(self)
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    /// Capacity weight suitable for RAM-cache admission, excluding metadata.
    pub fn pixel_capacity_bytes(&self) -> u64 {
        self.pixels.capacity_bytes()
    }

    pub fn pixels(&self) -> &RasterPixels {
        &self.pixels
    }
    pub fn color_space(&self) -> &RasterColorSpace {
        &self.color_space.space
    }

    /// Advisory sample units multiplier, retained separately from pixel data.
    /// Cursor hotspot in decoded-image pixel coordinates.
    pub fn hotspot(&self) -> Option<(u32, u32)> {
        self.hotspot
    }
    pub fn with_hotspot(mut self, hotspot: Option<(u32, u32)>) -> Result<Self, RasterError> {
        if hotspot.is_some_and(|(x, y)| x >= self.width || y >= self.height) {
            return Err(RasterError::InvalidHotspot);
        }
        self.hotspot = hotspot;
        Ok(self)
    }

    pub fn sample_scale(&self) -> f32 {
        self.sample_scale
    }

    pub fn with_sample_scale(mut self, scale: f32) -> Result<Self, RasterError> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(RasterError::InvalidScale);
        }
        self.sample_scale = scale;
        Ok(self)
    }

    /// Convert declared sRGB samples to straight-alpha linear sRGB floats.
    /// Alpha is coverage and never receives the RGB transfer function. HDR
    /// values are retained; no tone mapping or quantization is performed.
    /// ICC/unknown primaries need a separate qualified color transform.
    pub fn to_linear_srgb(&self) -> Result<Self, RasterError> {
        self.to_linear_srgb_with_budget(None)
    }

    pub fn to_linear_srgb_with_budget(
        &self,
        budget: Option<&rrrah_memory::MemoryBudget>,
    ) -> Result<Self, RasterError> {
        self.to_linear_srgb_with_budget_and_cancel(budget, || false)
    }

    /// Convert with cancellation checked at most every 4096 pixels.
    /// Cancellation drops partial output and its memory reservation.
    pub fn to_linear_srgb_with_budget_and_cancel(
        &self,
        budget: Option<&rrrah_memory::MemoryBudget>,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, RasterError> {
        if cancel() {
            return Err(RasterError::Cancelled);
        }
        let encoded = match self.color_space.space {
            RasterColorSpace::Srgb | RasterColorSpace::AssumedSrgb | RasterColorSpace::Bt709 => true,
            RasterColorSpace::LinearSrgb => false,
            _ => return Err(RasterError::ColorTransformRequired),
        };
        let bt709 = self.color_space.space == RasterColorSpace::Bt709;
        if !encoded && let RasterPixels::Rgba32Float(pixels) = &self.pixels {
            for (index, rgba) in pixels.as_chunks::<4>().0.iter().enumerate() {
                if index.is_multiple_of(4096) && cancel() {
                    return Err(RasterError::Cancelled);
                }
                if rgba.iter().any(|value| !value.is_finite()) {
                    return Err(RasterError::NonFiniteSample);
                }
                if !(0.0..=1.0).contains(&rgba[3]) {
                    return Err(RasterError::InvalidAlpha);
                }
            }
            if cancel() {
                return Err(RasterError::Cancelled);
            }
            return Ok(self.clone());
        }
        let count = self.pixels.len();
        let reservation = budget
            .map(|b| {
                b.try_reserve(
                    (count as u64)
                        .checked_mul(4)
                        .ok_or(rrrah_memory::BufferError::Overflow)?,
                )
            })
            .transpose()?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(count)
            .map_err(|_| RasterError::Allocation)?;
        let mut convert = |rgba: [f32; 4]| -> Result<(), RasterError> {
            if rgba.iter().any(|value| !value.is_finite()) {
                return Err(RasterError::NonFiniteSample);
            }
            if !(0.0..=1.0).contains(&rgba[3]) {
                return Err(RasterError::InvalidAlpha);
            }
            for &value in &rgba[..3] {
                let linear = if bt709 {
                    if value < 0.081 {
                        value / 4.5
                    } else {
                        ((value + 0.099) / 1.099).powf(1.0 / 0.45)
                    }
                } else if encoded {
                    srgb_linear(value)
                } else {
                    value
                };
                if !linear.is_finite() {
                    return Err(RasterError::NonFiniteSample);
                }
                output.push(linear);
            }
            output.push(rgba[3]);
            Ok(())
        };
        match &self.pixels {
            RasterPixels::Rgba8(pixels) if encoded && !bt709 => {
                let table = SRGB8_LINEAR.get_or_init(srgb8_table);
                for (index, rgba) in pixels.as_chunks::<4>().0.iter().enumerate() {
                    if index.is_multiple_of(4096) && cancel() {
                        return Err(RasterError::Cancelled);
                    }
                    output.extend_from_slice(&[
                        table[rgba[0] as usize],
                        table[rgba[1] as usize],
                        table[rgba[2] as usize],
                        f32::from(rgba[3]) / 255.0,
                    ]);
                }
            }
            RasterPixels::Rgba8(pixels) => {
                for (index, rgba) in pixels.as_chunks::<4>().0.iter().enumerate() {
                    if index.is_multiple_of(4096) && cancel() {
                        return Err(RasterError::Cancelled);
                    }
                    convert([
                        f32::from(rgba[0]) / 255.0,
                        f32::from(rgba[1]) / 255.0,
                        f32::from(rgba[2]) / 255.0,
                        f32::from(rgba[3]) / 255.0,
                    ])?;
                }
            }
            RasterPixels::Rgba16(pixels) => {
                for (index, rgba) in pixels.as_chunks::<4>().0.iter().enumerate() {
                    if index.is_multiple_of(4096) && cancel() {
                        return Err(RasterError::Cancelled);
                    }
                    convert([
                        f32::from(rgba[0]) / 65535.0,
                        f32::from(rgba[1]) / 65535.0,
                        f32::from(rgba[2]) / 65535.0,
                        f32::from(rgba[3]) / 65535.0,
                    ])?;
                }
            }
            RasterPixels::Rgba32Float(pixels) => {
                for (index, rgba) in pixels.as_chunks::<4>().0.iter().enumerate() {
                    if index.is_multiple_of(4096) && cancel() {
                        return Err(RasterError::Cancelled);
                    }
                    convert([rgba[0], rgba[1], rgba[2], rgba[3]])?;
                }
            }
        }
        if cancel() {
            return Err(RasterError::Cancelled);
        }
        Self::new(
            self.width,
            self.height,
            RasterPixels::Rgba32Float(match reservation {
                Some(reservation) => reservation.try_adopt(output)?.into(),
                None => Arc::new(output).into(),
            }),
            RasterColorSpace::LinearSrgb,
        )?
        .with_sample_scale(self.sample_scale)?
        .with_hotspot(self.hotspot)?
        .with_image_selection(self.image_index, self.image_count)
    }
}

#[cfg(test)]
mod fast_display_tests {
    use super::*;
    #[test]
    fn hlg_policy_refusals_cancellation_and_black_release_credit() {
        let source = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![0., 0., 0., 0.5]).into()),
            RasterColorSpace::Cicp {
                primaries: 9,
                transfer: 18,
                matrix: 9,
                full_range: true,
            },
        )
        .unwrap();
        let budget = rrrah_memory::MemoryBudget::new(16);
        for (peak, gamma) in [
            (0., 1.2),
            (f32::NAN, 1.2),
            (10001., 1.2),
            (1000., 0.),
            (1000., f32::INFINITY),
            (1000., 4.1),
        ] {
            assert!(
                source
                    .hlg_to_linear_srgb_with_budget_and_cancel(100., peak, gamma, Some(&budget), || false)
                    .is_err()
            );
            assert_eq!(budget.used(), 0);
        }
        let black = source
            .hlg_to_linear_srgb_with_budget_and_cancel(100., 1000., 0.8, Some(&budget), || false)
            .unwrap();
        let RasterPixels::Rgba32Float(pixels) = black.pixels() else {
            panic!()
        };
        assert_eq!(pixels.as_ref(), &[0., 0., 0., 0.5]);
        drop(black);
        assert_eq!(budget.used(), 0);
        assert!(
            source
                .hlg_to_linear_srgb_with_budget_and_cancel(
                    100.,
                    1000.,
                    1.2,
                    Some(&rrrah_memory::MemoryBudget::new(15)),
                    || false
                )
                .is_err()
        );
        let calls = std::cell::Cell::new(0);
        assert!(matches!(
            source.hlg_to_linear_srgb_with_budget_and_cancel(100., 1000., 1.2, Some(&budget), || {
                calls.set(calls.get() + 1);
                calls.get() >= 3
            }),
            Err(RasterError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn hlg_colors_match_independent_decimal_display_oracle() {
        let oracle = include_str!("../../../tests/fixtures/raster/hlg-color-decimal-oracle.tsv");
        let budget = rrrah_memory::MemoryBudget::new(16);
        let mut count = 0;
        for line in oracle.lines() {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 11);
            let policy: Vec<f32> = fields[..3].iter().map(|v| v.parse().unwrap()).collect();
            let input: Vec<u16> = fields[3..7].iter().map(|v| v.parse().unwrap()).collect();
            let expected: Vec<f64> = fields[7..].iter().map(|v| v.parse().unwrap()).collect();
            let source = DecodedRaster::new(
                1,
                1,
                RasterPixels::Rgba16(Arc::new(input).into()),
                RasterColorSpace::Cicp {
                    primaries: 9,
                    transfer: 18,
                    matrix: 9,
                    full_range: true,
                },
            )
            .unwrap();
            let converted = source
                .hlg_to_linear_srgb_with_budget_and_cancel(
                    policy[0],
                    policy[1],
                    policy[2],
                    Some(&budget),
                    || false,
                )
                .unwrap();
            let RasterPixels::Rgba32Float(values) = converted.pixels() else {
                panic!()
            };
            for channel in 0..4 {
                let tolerance = 1e-7 + expected[channel].abs() * 1e-6;
                assert!(
                    (f64::from(values[channel]) - expected[channel]).abs() <= tolerance,
                    "HLG vector {count}, channel {channel}: {} vs {}",
                    values[channel],
                    expected[channel]
                );
            }
            drop(converted);
            assert_eq!(budget.used(), 0);
            assert!(
                source
                    .pq_to_linear_srgb_with_budget_and_cancel(policy[0], Some(&budget), || false)
                    .is_err()
            );
            count += 1;
        }
        assert_eq!(count, 267);
    }
    #[test]
    fn pq_colors_match_independent_decimal_and_chromaticity_oracle() {
        let oracle = include_str!("../../../tests/fixtures/raster/pq-color-decimal-oracle.tsv");
        let budget = rrrah_memory::MemoryBudget::new(16);
        let mut count = 0;
        for line in oracle.lines() {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 9);
            let white = fields[0].parse().unwrap();
            let input: Vec<u16> = fields[1..5].iter().map(|v| v.parse().unwrap()).collect();
            let expected: Vec<f64> = fields[5..].iter().map(|v| v.parse().unwrap()).collect();
            let source = DecodedRaster::new(
                1,
                1,
                RasterPixels::Rgba16(Arc::new(input).into()),
                RasterColorSpace::Cicp {
                    primaries: 9,
                    transfer: 16,
                    matrix: 9,
                    full_range: true,
                },
            )
            .unwrap();
            let converted = source
                .pq_to_linear_srgb_with_budget_and_cancel(white, Some(&budget), || false)
                .unwrap();
            let RasterPixels::Rgba32Float(values) = converted.pixels() else {
                panic!()
            };
            for channel in 0..4 {
                let tolerance = 1e-7 + expected[channel].abs() * 1e-6;
                assert!(
                    (f64::from(values[channel]) - expected[channel]).abs() <= tolerance,
                    "vector {count}, channel {channel}: {} vs {}",
                    values[channel],
                    expected[channel]
                );
            }
            drop(converted);
            assert_eq!(budget.used(), 0);
            count += 1;
        }
        assert_eq!(count, 445);
    }
    #[test]
    fn pq_absolute_luminance_gamut_alpha_and_budget_contract() {
        let color = RasterColorSpace::Cicp {
            primaries: 9,
            transfer: 16,
            matrix: 9,
            full_range: true,
        };
        let source = DecodedRaster::new(
            5,
            1,
            RasterPixels::Rgba32Float(
                Arc::new(vec![
                    0., 0., 0., 0.25, 0.50807842, 0.50807842, 0.50807842, 0.5, 0.7518271, 0.7518271,
                    0.7518271, 0.75, 1., 1., 1., 1., 1., 0., 0., 1.,
                ])
                .into(),
            ),
            color,
        )
        .unwrap();
        let budget = rrrah_memory::MemoryBudget::new(80);
        let output = source
            .pq_to_linear_srgb_with_budget_and_cancel(100., Some(&budget), || false)
            .unwrap();
        let RasterPixels::Rgba32Float(p) = output.pixels() else {
            panic!()
        };
        for (i, expected) in [0., 1., 10., 100.].into_iter().enumerate() {
            for value in &p[i * 4..i * 4 + 3] {
                assert!((*value - expected).abs() < 0.001);
            }
            assert_eq!(p[i * 4 + 3], [0.25, 0.5, 0.75, 1.][i]);
        }
        assert!(p[16] > 100. && p[17] < 0. && p[18] < 0.);
        assert_eq!(output.color_space(), &RasterColorSpace::LinearSrgb);
        assert_eq!(budget.used(), 80);
        drop(output);
        assert_eq!(budget.used(), 0);
        assert!(
            source
                .pq_to_linear_srgb_with_budget_and_cancel(
                    100.,
                    Some(&rrrah_memory::MemoryBudget::new(79)),
                    || false
                )
                .is_err()
        );
        let calls = std::cell::Cell::new(0);
        assert!(matches!(
            source.pq_to_linear_srgb_with_budget_and_cancel(100., Some(&budget), || {
                calls.set(calls.get() + 1);
                calls.get() >= 3
            }),
            Err(RasterError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        for white in [0., f32::NAN, f32::INFINITY, 10001.] {
            assert!(
                source
                    .pq_to_linear_srgb_with_budget_and_cancel(white, Some(&budget), || false)
                    .is_err()
            );
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn srgb8_table_matches_previous_formula_bit_for_bit() {
        let pixels: Vec<u8> = (0..=255u8).flat_map(|v| [v, 255 - v, v / 2, v]).collect();
        let source = DecodedRaster::new(
            256,
            1,
            RasterPixels::Rgba8(Arc::new(pixels.clone()).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap();
        let prepared = source.to_linear_srgb().unwrap();
        let RasterPixels::Rgba32Float(values) = prepared.pixels() else {
            panic!()
        };
        for (raw, linear) in pixels.chunks_exact(4).zip(values.chunks_exact(4)) {
            for channel in 0..3 {
                let v = f32::from(raw[channel]) / 255.;
                let previous = if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                };
                assert_eq!(linear[channel].to_bits(), previous.to_bits());
            }
            assert_eq!(linear[3].to_bits(), (f32::from(raw[3]) / 255.).to_bits());
        }
    }
    #[test]
    fn validated_linear_float_display_shares_storage_and_preserves_metadata() {
        let pixels = Arc::new(vec![2., -0.5, 1., 0.25, 0., 0., 0., 1.]);
        let source = DecodedRaster::new(
            2,
            1,
            RasterPixels::Rgba32Float(pixels.clone().into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .with_sample_scale(2.5)
        .unwrap()
        .with_hotspot(Some((1, 0)))
        .unwrap()
        .with_image_selection(1, 3)
        .unwrap();
        let prepared = source.to_linear_srgb().unwrap();
        let RasterPixels::Rgba32Float(values) = prepared.pixels() else {
            panic!()
        };
        assert!(std::ptr::eq(pixels.as_ptr(), values.as_ptr()));
        assert_eq!(prepared.sample_scale(), 2.5);
        assert_eq!(prepared.hotspot(), Some((1, 0)));
        assert_eq!((prepared.image_index(), prepared.image_count()), (1, 3));
        for sample in [f32::NAN, f32::INFINITY] {
            let mut invalid = (*pixels).clone();
            invalid[0] = sample;
            let source = DecodedRaster::new(
                2,
                1,
                RasterPixels::Rgba32Float(Arc::new(invalid).into()),
                RasterColorSpace::LinearSrgb,
            )
            .unwrap();
            assert!(matches!(
                source.to_linear_srgb(),
                Err(RasterError::NonFiniteSample)
            ));
        }
        let source = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![0., 0., 0., 2.]).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        assert!(matches!(source.to_linear_srgb(), Err(RasterError::InvalidAlpha)));
    }
}

#[derive(Debug, Error)]
pub enum RasterError {
    #[error("HDR reference white must be finite and between 1 and 10000 cd/m²")]
    InvalidHdrReferenceWhite,
    #[error("HDR encoded RGB codes must be in the range zero to one")]
    InvalidPqSample,
    #[error("HLG peak must be finite and 1–10000 cd/m²; system gamma must be finite and 0.1–4")]
    InvalidHlgDisplayPolicy,
    #[error("raster conversion cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] rrrah_memory::BufferError),
    #[error("image selection must lie within a nonempty container")]
    InvalidImageSelection,
    #[error("cursor hotspot lies outside image bounds")]
    InvalidHotspot,
    #[error("raster needs an ICC or source-primary transform before display")]
    ColorTransformRequired,
    #[error("raster float samples must be finite before display")]
    NonFiniteSample,
    #[error("straight alpha must be in the range zero to one")]
    InvalidAlpha,
    #[error("could not allocate linear raster output")]
    Allocation,
    #[error("sample scale must be finite and positive")]
    InvalidScale,
    #[error("raster dimensions must be nonzero")]
    Empty,
    #[error("raster dimensions overflow")]
    Dimensions,
    #[error("expected {expected} RGBA samples, got {actual}")]
    SampleCount { expected: usize, actual: usize },
}

#[cfg(test)]
mod tests {
    #[test]
    fn bt709_all_u16_values_match_f64_reference_and_release_managed_buffers() {
        let budget = rrrah_memory::MemoryBudget::new(65536 * 24);
        let mut samples = budget.try_buffer(65536 * 4, 0u16).unwrap();
        for (value, rgba) in samples.chunks_exact_mut(4).enumerate() {
            rgba.fill(u16::try_from(value).unwrap());
        }
        let source = super::DecodedRaster::new(
            65536,
            1,
            super::RasterPixels::Rgba16(samples.freeze().into()),
            super::RasterColorSpace::Bt709,
        )
        .unwrap();
        let linear = source.to_linear_srgb_with_budget(Some(&budget)).unwrap();
        let super::RasterPixels::Rgba32Float(output) = linear.pixels() else {
            panic!("float output");
        };
        for (value, rgba) in output.chunks_exact(4).enumerate() {
            let encoded = f64::from(u16::try_from(value).unwrap()) / 65535.0;
            let expected = if encoded < 0.081 {
                encoded / 4.5
            } else {
                ((encoded + 0.099) / 1.099).powf(1.0 / 0.45)
            };
            for &channel in &rgba[..3] {
                assert!((f64::from(channel) - expected).abs() < 3e-7, "sample {value}");
            }
            assert!((f64::from(rgba[3]) - encoded).abs() < 3e-8);
        }
        assert_eq!(linear.color_space(), &super::RasterColorSpace::LinearSrgb);
        drop(linear);
        drop(source);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn exclusive_native_mutation_preserves_managed_credit_and_refuses_shared_owners() {
        let budget = rrrah_memory::MemoryBudget::new(16);
        let pixels = budget.try_buffer(4, 0u16).unwrap().freeze();
        let mut raster = super::DecodedRaster::new(
            1,
            1,
            super::RasterPixels::Rgba16(pixels.into()),
            super::RasterColorSpace::Srgb,
        )
        .unwrap();
        let other = raster.clone();
        assert!(raster.rgba16_mut().is_none());
        assert!(raster.rgba8_mut().is_none());
        assert_eq!(budget.used(), 8);
        drop(other);
        raster
            .rgba16_mut()
            .unwrap()
            .copy_from_slice(&[65535, 32768, 1, 65535]);
        assert_eq!(budget.used(), 8);
        let super::RasterPixels::Rgba16(p) = raster.pixels() else {
            panic!()
        };
        assert_eq!(p.as_slice(), &[65535, 32768, 1, 65535]);
        drop(raster);
        assert_eq!(budget.used(), 0);
    }
    use super::*;

    #[test]
    fn image_selection_is_bounded_and_survives_color_preparation() {
        let frame = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba8(Arc::new(vec![1, 2, 3, 255]).into()),
            RasterColorSpace::AssumedSrgb,
        )
        .unwrap();
        assert_eq!((frame.image_index(), frame.image_count()), (0, 1));
        for (index, count) in [(0, 0), (1, 1), (usize::MAX, usize::MAX)] {
            assert!(matches!(
                frame.clone().with_image_selection(index, count),
                Err(RasterError::InvalidImageSelection)
            ));
        }
        let prepared = frame
            .with_image_selection(2, 3)
            .unwrap()
            .to_linear_srgb()
            .unwrap();
        assert_eq!((prepared.image_index(), prepared.image_count()), (2, 3));
    }

    #[test]
    fn srgb_gray_is_linearized_but_alpha_stays_coverage() {
        let raster = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba8(Arc::new(vec![128, 128, 128, 128]).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap();
        let linear = raster.to_linear_srgb().unwrap();
        let RasterPixels::Rgba32Float(p) = linear.pixels() else {
            panic!("wrong precision")
        };
        assert!((p[0] - 0.2158605).abs() < 1e-6);
        assert!((p[3] - 128.0 / 255.0).abs() < 1e-7);
        assert_eq!(linear.color_space(), &RasterColorSpace::LinearSrgb);
    }

    #[test]
    fn linear_hdr_remains_unclipped_and_unpremultiplied() {
        let samples = vec![4.0, -0.5, 0.25, 0.0];
        let raster = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(samples.clone()).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .with_sample_scale(2.5)
        .unwrap();
        let linear = raster.to_linear_srgb().unwrap();
        let RasterPixels::Rgba32Float(p) = linear.pixels() else {
            panic!("wrong precision")
        };
        assert_eq!(p.as_slice(), samples);
        assert_eq!(linear.sample_scale(), 2.5);
    }

    #[test]
    fn unqualified_colors_and_invalid_alpha_are_rejected() {
        for color in [
            RasterColorSpace::Unspecified,
            RasterColorSpace::LinearRgbUnspecified,
            RasterColorSpace::Icc(vec![]),
        ] {
            let frame =
                DecodedRaster::new(1, 1, RasterPixels::Rgba8(Arc::new(vec![0; 4]).into()), color).unwrap();
            assert!(matches!(
                frame.to_linear_srgb(),
                Err(RasterError::ColorTransformRequired)
            ));
        }
        for samples in [vec![0.0, 0.0, 0.0, 2.0], vec![f32::NAN, 0.0, 0.0, 1.0]] {
            let frame = DecodedRaster::new(
                1,
                1,
                RasterPixels::Rgba32Float(Arc::new(samples).into()),
                RasterColorSpace::LinearSrgb,
            )
            .unwrap();
            assert!(frame.to_linear_srgb().is_err());
        }
    }

    #[test]
    fn validates_shape_and_retains_hdr_and_alpha() {
        assert!(
            DecodedRaster::new(
                0,
                1,
                RasterPixels::Rgba8(Arc::new(vec![]).into()),
                RasterColorSpace::Srgb
            )
            .is_err()
        );
        assert!(
            DecodedRaster::new(
                2,
                1,
                RasterPixels::Rgba8(Arc::new(vec![0; 4]).into()),
                RasterColorSpace::Srgb
            )
            .is_err()
        );
        let frame = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![4.0, -0.5, 1.0, 0.25]).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let RasterPixels::Rgba32Float(pixels) = frame.pixels() else {
            panic!("precision lost")
        };
        assert_eq!(pixels.as_slice(), [4.0, -0.5, 1.0, 0.25]);
    }
    #[test]
    fn raster_capacity_counts_spare_storage_and_preserves_hdr_bits() {
        let mut u8s = Vec::with_capacity(64);
        u8s.extend_from_slice(&[1_u8, 2, 3, 255]);
        let u8s = Arc::new(u8s);
        let raster = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba8(u8s.clone().into()),
            RasterColorSpace::Srgb,
        )
        .unwrap();
        assert_eq!(raster.pixel_capacity_bytes(), u8s.capacity() as u64);
        let u16s = Arc::new(vec![0_u16, 32768, 65535, 65535]);
        assert_eq!(
            RasterPixels::Rgba16(u16s.clone().into()).capacity_bytes(),
            (u16s.capacity() * 2) as u64
        );
        let values = Arc::new(vec![-0.0_f32, 0.125, 128.0, 1.0]);
        let bits: Vec<_> = values.iter().map(|value| value.to_bits()).collect();
        let raster = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(values.clone().into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        assert_eq!(raster.pixel_capacity_bytes(), (values.capacity() * 4) as u64);
        let clone = raster.clone();
        let RasterPixels::Rgba32Float(samples) = clone.pixels() else {
            panic!("sample type changed")
        };
        assert!(std::ptr::eq(samples.as_ptr(), values.as_ptr()));
        assert_eq!(
            samples.iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
            bits
        );
    }
}

#[cfg(test)]
mod managed_ownership_tests {
    use super::*;
    #[test]
    fn reservation_follows_last_raster_owner_and_preserves_hdr_bits() {
        let budget = rrrah_memory::MemoryBudget::new(64);
        let mut samples = Vec::with_capacity(16);
        samples.extend([4.0, -0.0, f32::from_bits(0x7fc00001), 1.0]);
        let ptr = samples.as_ptr();
        let raster = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(samples).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .try_manage_pixels(&budget)
        .unwrap();
        let RasterPixels::Rgba32Float(values) = &raster.pixels else {
            unreachable!()
        };
        assert_eq!(values.as_ptr(), ptr);
        assert_eq!(values[2].to_bits(), 0x7fc00001);
        assert_eq!(values[1].to_bits(), (-0.0f32).to_bits());
        assert_eq!(budget.used(), 64);
        let retained = raster.clone();
        drop(raster);
        assert_eq!(budget.used(), 64);
        drop(retained);
        assert_eq!(budget.used(), 0);
    }
}
