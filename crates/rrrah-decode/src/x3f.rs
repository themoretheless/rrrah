//! Borrowed X3F 2.0–2.2 inventory, properties and legacy sensor channels.
//! Foveon external specification: https://libopenraw.freedesktop.org/formats/x3f/x3f-raw-format.pdf

#[derive(Debug, thiserror::Error)]
pub enum X3fError {
    #[error("invalid or truncated X3F structure")]
    Invalid,
    #[error("unsupported X3F container version")]
    Version,
    #[error("X3F Auto white balance requires enabled embedded AutoRGBNeutral calibration")]
    UnsupportedAutoWhiteBalance,
    #[error("X3F directory exceeds 1024 entries")]
    Limit,
    #[error("X3F inventory cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
}

#[derive(Debug, Clone, Copy)]
pub struct X3fEntry<'a> {
    pub kind: [u8; 4],
    pub offset: u32,
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X3fImageInfo {
    pub image_type: u32,
    pub encoding: u32,
    pub width: u32,
    pub height: u32,
    pub row_bytes: u32,
}
/// Borrowed legacy type-3/encoding-6 Huffman tables and independently bounded rows.
/// Does not decode samples or apply legacy channel offsets/calibration.
#[derive(Debug)]
pub struct X3fLegacyHuffman<'a> {
    pub info: X3fImageInfo,
    mapping: &'a [u8],
    codes: &'a [u8],
    stream: &'a [u8],
    offsets: &'a [u8],
}
#[derive(Debug)]
pub struct X3fLegacyChannels {
    pub width: u32,
    pub height: u32,
    /// Automatic legacy predictor offset, before camera color calibration.
    pub legacy_offset: i16,
    /// Three interleaved camera channels; these are not display RGB.
    pub pixels: rrrah_core::PixelBuffer<u16>,
}
impl X3fLegacyChannels {
    /// Restore signed predictor samples modulo u16 before camera calibration.
    /// The original unpacked buffer and its explicitly reported offset remain intact.
    pub fn unshifted(
        &self,
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Self, X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if self.legacy_offset == 0 {
            return Ok(Self {
                width: self.width,
                height: self.height,
                legacy_offset: 0,
                pixels: self.pixels.clone(),
            });
        }
        let mut output = budget.try_buffer(self.pixels.len(), 0u16)?;
        for (source, target) in self.pixels.chunks(768).zip(output.chunks_mut(768)) {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            for (value, dest) in source.iter().zip(target) {
                *dest = value.wrapping_sub(self.legacy_offset as u16);
            }
        }
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        Ok(Self {
            width: self.width,
            height: self.height,
            legacy_offset: 0,
            pixels: output.freeze().into(),
        })
    }
    /// Complete supported legacy Auto processing with explicit target and crop.
    /// Returned channels retain the legacy transform scale, not an encoded RGB
    /// transfer function. Caller owns target/crop policy and display conversion.
    pub fn process_auto(
        &self,
        calibration: &[u8],
        target_from_xyz: [[f32; 3]; 3],
        area: [u32; 4],
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<i32>, X3fError> {
        self.process_with_white_balance(calibration, b"Auto", target_from_xyz, area, budget, &mut cancelled)
    }
    /// Process legacy camera channels using an explicitly selected calibrated WB mode.
    pub fn process_with_white_balance(
        &self,
        calibration: &[u8],
        mode: &[u8],
        target_from_xyz: [[f32; 3]; 3],
        area: [u32; 4],
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<i32>, X3fError> {
        let [left, top, right, bottom] = area;
        if left >= right || top >= bottom || right > self.width || bottom > self.height {
            return Err(X3fError::Invalid);
        }
        let transform = x3f_color_transform_for_mode(calibration, mode, target_from_xyz, &mut cancelled)?;
        let neutral = x3f_neutral_response(calibration, mode, &mut cancelled)?;
        let unshifted = self.unshifted(budget, &mut cancelled)?;
        let corrected = unshifted.correct_sensor_channels(calibration, neutral, budget, &mut cancelled)?;
        drop(unshifted);
        let mut work = budget.try_buffer(corrected.len(), 0i32)?;
        for (source, target) in corrected.chunks(768).zip(work.chunks_mut(768)) {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            target.copy_from_slice(source);
        }
        drop(corrected);
        repair_x3f_bad_pixels_from_camf(&mut work, self.width, self.height, calibration, &mut cancelled)?;
        sharpen_x3f_red(&mut work, self.width, self.height, budget, &mut cancelled)?;
        linearize_x3f_highlights_for_mode(&mut work, calibration, mode, &mut cancelled)?;
        let curves = x3f_noise_curves_for_mode(calibration, mode, transform.luminance, budget, &mut cancelled)?;
        smooth_x3f_hues(
            &mut work,
            self.width,
            self.height,
            &curves[7],
            budget,
            &mut cancelled,
        )?;
        smooth_x3f_hues_wide(
            &mut work,
            self.width,
            self.height,
            &curves[6],
            budget,
            &mut cancelled,
        )?;
        transform_x3f_pixels(
            &mut work,
            [&curves[0], &curves[1], &curves[2]],
            &transform,
            &mut cancelled,
        )?;
        smooth_x3f_chroma(
            &mut work,
            self.width,
            self.height,
            [&curves[3], &curves[4], &curves[5]],
            budget,
            &mut cancelled,
        )?;
        drop(curves);
        if area == [0, 0, self.width, self.height] {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            Ok(work.freeze().into())
        } else {
            crop_x3f_channels(&work, self.width, self.height, area, budget, &mut cancelled)
        }
    }
    /// Sensor-stage Auto WB from embedded neutral or calibrated matrix derivation.
    /// Final display color remains a separate stage.
    pub fn correct_sensor_auto(
        &self,
        calibration: &[u8],
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<i32>, X3fError> {
        let response = x3f_neutral_response(calibration, b"Auto", &mut cancelled)?;
        self.correct_sensor_channels(calibration, response, budget, &mut cancelled)
    }

    /// Applies sensor corrections into managed signed camera channels.
    /// Requires an explicitly resolved neutral response, not display RGB.
    pub fn correct_sensor_channels(
        &self,
        calibration: &[u8],
        neutral: [f32; 3],
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<i32>, X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if neutral.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(X3fError::Invalid);
        }
        let black = self.processed_black_from_camf(calibration, budget, &mut cancelled)?;
        let drift = unique_camf_matrix(calibration, b"DarkDrift", &mut cancelled)?;
        let polynomial = unique_camf_matrix(calibration, b"PostPolyMatrix", &mut cancelled)?;
        let gain = unique_camf_matrix(calibration, b"SpatialGain", &mut cancelled)?;
        let filter = if x3f_block_enabled(calibration, b"ColumnFilter", &mut cancelled)? {
            let matrix = unique_camf_matrix(calibration, b"ColumnFilter", &mut cancelled)?;
            if matrix.elements != 1 {
                return Err(X3fError::Invalid);
            }
            matrix.float32(0)?
        } else {
            0.0
        };
        if gain.dimensions.len() != 36 {
            return Err(X3fError::Invalid);
        }
        let columns = word(gain.dimensions, 12);
        if columns < 2 || self.width < 2 {
            return Err(X3fError::Invalid);
        }
        let spacing = (u64::from(self.width) + u64::from(columns) - 2) / (u64::from(columns) - 1);
        let maximum = neutral.iter().copied().fold(0f32, f32::max);
        let neutral = neutral.map(|v| v / maximum);
        if neutral.iter().any(|v| *v == 0.0) {
            return Err(X3fError::Invalid);
        }
        polynomial.post_polynomial([0; 3])?;
        gain.spatial_gain(0.0, 0.0)?;
        let mut output = budget.try_buffer(self.pixels.len(), 0i32)?;
        for row in 0..self.height {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            let coefficients = drift.dark_drift(row, self.height)?;
            let base = row as usize * self.width as usize * 3;
            let mut previous = [
                self.pixels[base] as i16,
                self.pixels[base + 1] as i16,
                self.pixels[base + 2] as i16,
            ];
            for col in 0..self.width {
                if col % 256 == 0 && cancelled() {
                    return Err(X3fError::Cancelled);
                }
                let index = base + col as usize * 3;
                let mut pixel = [0i32; 3];
                for c in 0..3 {
                    let current = self.pixels[index + c] as i16;
                    let delta = i64::from(current) - i64::from(previous[c]);
                    previous[c] = current;
                    let filtered =
                        f64::from((delta + (delta * delta >> 14)) as f32 * filter - coefficients[c][1])
                            - f64::from(coefficients[c][0])
                                * (f64::from(col as f32 / self.width as f32) - 0.5)
                            - f64::from(black[row as usize * 3 + c]);
                    let value = f64::from(current) + filtered.floor();
                    if !value.is_finite() || value < i32::MIN as f64 || value > i32::MAX as f64 {
                        return Err(X3fError::Limit);
                    }
                    pixel[c] = value as i32;
                }
                let pixel = polynomial.post_polynomial(pixel)?;
                let grid_rows = word(gain.dimensions, 0);
                let gy = (f64::from(row) / f64::from(self.height - 1) * f64::from(grid_rows - 1)) as f32;
                let top = (gy as u32).min(grid_rows - 2);
                let ty = gy - top as f32;
                let left = u64::from(col) / spacing;
                let remainder = u64::from(col) % spacing;
                let mut gains = [0f32; 3];
                for c in 0..3 {
                    let at = |y: u32, x: u64| {
                        gain.float32(((u64::from(y) * u64::from(columns) + x) * 3 + c as u64) as usize)
                    };
                    let a = at(top, left)? * (1.0 - ty) + at(top + 1, left)? * ty;
                    let b = at(top, left + 1)? * (1.0 - ty) + at(top + 1, left + 1)? * ty;
                    gains[c] = a * (spacing - remainder) as f32 + b * remainder as f32;
                    if !gains[c].is_finite() || gains[c] <= 0.0 {
                        return Err(X3fError::Invalid);
                    }
                }
                for c in 0..3 {
                    let value =
                        (f64::from(pixel[c]) * f64::from(gains[c]) / spacing as f64 / f64::from(neutral[c]))
                            .floor()
                            .min(32000.0);
                    if !value.is_finite() || value < i32::MIN as f64 {
                        return Err(X3fError::Limit);
                    }
                    output[index + c] = value as i32;
                }
            }
        }
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        drop(black);
        Ok(output.freeze().into())
    }

    /// Resolves black-processing parameters from the file's CAMF metadata.
    /// Shield-derived drift fallback is not implemented; it fails explicitly.
    pub fn processed_black_from_camf(
        &self,
        calibration: &[u8],
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<f32>, X3fError> {
        if !x3f_block_enabled(calibration, b"DarkDrift", &mut cancelled)? {
            return Err(X3fError::Invalid);
        }
        let drift = unique_camf_matrix(calibration, b"DarkDrift", &mut cancelled)?;
        let ranges = unique_camf_matrix(calibration, b"DarkShieldColRange", &mut cancelled)?;
        if ranges.element_type != 2 || ranges.elements != 4 || ranges.data.len() != 16 {
            return Err(X3fError::Invalid);
        }
        let ranges = [
            [word(ranges.data, 0), word(ranges.data, 4)],
            [word(ranges.data, 8), word(ranges.data, 12)],
        ];
        let filter = if x3f_block_enabled(calibration, b"ColumnFilter", &mut cancelled)? {
            let matrix = unique_camf_matrix(calibration, b"ColumnFilter", &mut cancelled)?;
            if matrix.elements != 1 {
                return Err(X3fError::Invalid);
            }
            matrix.float32(0)?
        } else {
            0.0
        };
        self.processed_black_rows(ranges, filter, &drift, budget, &mut cancelled)
    }

    /// Complete qualified black-row arithmetic, returned in managed storage.
    pub fn processed_black_rows(
        &self,
        ranges: [[u32; 2]; 2],
        filter: f32,
        drift: &X3fCamfMatrix<'_>,
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<f32>, X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if self.height < 22 {
            return Err(X3fError::Invalid);
        }
        let initial = self.black_rows(ranges, filter, drift, budget, &mut cancelled)?;
        let mut work = budget.try_buffer(initial.len(), 0f32)?;
        for (source, target) in initial.chunks(768).zip(work.chunks_mut(768)) {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            target.copy_from_slice(source);
        }
        drop(initial);
        let mean = smooth_x3f_black_rows(&mut work, &mut cancelled)?;
        self.adjust_black_scene(&mut work, mean, &mut cancelled)?;
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        Ok(work.freeze().into())
    }

    /// Adds the legacy scene-sampled correction to already smoothed black rows.
    /// The caller supplies the forward mean returned by smooth_x3f_black_rows.
    pub fn adjust_black_scene(
        &self,
        black: &mut [f32],
        mean: [f32; 3],
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<[f32; 3], X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if self.width < 3
            || self.height < 3
            || black.len() as u64 != u64::from(self.height) * 3
            || self.pixels.len() as u64 != u64::from(self.width) * u64::from(self.height) * 3
            || mean.iter().any(|v| !v.is_finite())
        {
            return Err(X3fError::Invalid);
        }
        let mut sums = [0i64; 3];
        let mut count = 0u64;
        for row in (2..self.height).step_by(4) {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            for col in (2..self.width).step_by(4) {
                if col % 256 == 2 && cancelled() {
                    return Err(X3fError::Cancelled);
                }
                let index = ((u64::from(row) * u64::from(self.width) + u64::from(col)) * 3) as usize;
                for c in 0..3 {
                    sums[c] = sums[c]
                        .checked_add(i64::from(self.pixels[index + c] as i16))
                        .ok_or(X3fError::Limit)?;
                }
                count += 1;
            }
        }
        let mut correction = [0f64; 3];
        for c in 0..3 {
            correction[c] = f64::from(mean[c] / 2.0) + sums[c] as f64 / (count as f64 * 100.0);
        }
        for chunk in black.chunks(768) {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            for (i, v) in chunk.iter().enumerate() {
                if !v.is_finite() || !((f64::from(*v) + correction[i % 3]) as f32).is_finite() {
                    return Err(X3fError::Invalid);
                }
            }
        }
        for (i, v) in black.iter_mut().enumerate() {
            if i % 768 == 0 && cancelled() {
                return Err(X3fError::Cancelled);
            }
            *v = (f64::from(*v) + correction[i % 3]) as f32;
        }
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        Ok(correction.map(|v| v as f32))
    }

    /// Initial per-row black estimate; smoothing is a separate processing stage.
    pub fn black_rows(
        &self,
        ranges: [[u32; 2]; 2],
        filter: f32,
        drift: &X3fCamfMatrix<'_>,
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<f32>, X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if self.height < 2 {
            return Err(X3fError::Invalid);
        }
        // Validate before admission, including both ranges and calibration shape.
        self.shield_column_mean(0, ranges[0], filter, &mut cancelled)?;
        self.shield_column_mean(0, ranges[1], filter, &mut cancelled)?;
        drift.dark_drift(0, self.height)?;
        let count = usize::try_from(u64::from(self.height) * 3).map_err(|_| X3fError::Limit)?;
        let mut result = budget.try_buffer(count, 0f32)?;
        for row in 0..self.height {
            let a = self.shield_column_mean(row, ranges[0], filter, &mut cancelled)?;
            let b = self.shield_column_mean(row, ranges[1], filter, &mut cancelled)?;
            let coefficients = drift.dark_drift(row, self.height)?;
            for channel in 0..3 {
                let value = (a[channel] + 3.0 * b[channel] - coefficients[channel][0]) / 4.0
                    - coefficients[channel][1];
                if !value.is_finite() {
                    return Err(X3fError::Invalid);
                }
                result[row as usize * 3 + channel] = value;
            }
        }
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        Ok(result.freeze().into())
    }

    /// Filtered signed shield-column mean, dropping one minimum and maximum.
    pub fn shield_column_mean(
        &self,
        row: u32,
        range: [u32; 2],
        filter: f32,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<[f32; 3], X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        let [start, end] = range;
        if row >= self.height
            || start == 0
            || start >= end
            || end >= self.width
            || !filter.is_finite()
            || self.pixels.len() as u64 != u64::from(self.width) * u64::from(self.height) * 3
        {
            return Err(X3fError::Invalid);
        }
        let mut sums = [0f32; 3];
        let mut minima = [f32::INFINITY; 3];
        let mut maxima = [f32::NEG_INFINITY; 3];
        for column in start..=end {
            if (column - start) % 256 == 0 && cancelled() {
                return Err(X3fError::Cancelled);
            }
            let index = ((u64::from(row) * u64::from(self.width) + u64::from(column)) * 3) as usize;
            for channel in 0..3 {
                let current = self.pixels[index + channel] as i16 as f32;
                let previous = self.pixels[index - 3 + channel] as i16 as f32;
                let value = current + (current - previous) * filter;
                if !value.is_finite() {
                    return Err(X3fError::Invalid);
                }
                sums[channel] += value;
                minima[channel] = minima[channel].min(value);
                maxima[channel] = maxima[channel].max(value);
            }
        }
        let count = end - start + 1;
        for channel in 0..3 {
            sums[channel] = if count == 2 {
                sums[channel] / 2.0
            } else {
                (sums[channel] - minima[channel] - maxima[channel]) / (count - 2) as f32
            };
            if !sums[channel].is_finite() {
                return Err(X3fError::Invalid);
            }
        }
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        Ok(sums)
    }

    /// Mean signed legacy camera channels within an inclusive shield rectangle.
    /// Coordinates must already be mapped into the decoded sensor frame.
    pub fn shield_mean(
        &self,
        rectangle: [u32; 4],
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<[f64; 3], X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        let [left, top, right, bottom] = rectangle;
        if left > right || top > bottom || right >= self.width || bottom >= self.height {
            return Err(X3fError::Invalid);
        }
        let expected = u64::from(self.width) * u64::from(self.height) * 3;
        if self.pixels.len() as u64 != expected {
            return Err(X3fError::Invalid);
        }
        let mut sums = [0i64; 3];
        let count = u64::from(right - left + 1) * u64::from(bottom - top + 1);
        for row in top..=bottom {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            for column in left..=right {
                if (column - left) % 256 == 0 && cancelled() {
                    return Err(X3fError::Cancelled);
                }
                let index = ((u64::from(row) * u64::from(self.width) + u64::from(column)) * 3) as usize;
                for channel in 0..3 {
                    sums[channel] = sums[channel]
                        .checked_add(i64::from(self.pixels[index + channel] as i16))
                        .ok_or(X3fError::Limit)?;
                }
            }
        }
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        Ok(sums.map(|sum| sum as f64 / count as f64))
    }
}
/// Decode legacy X3F using the unique explicit file WB_DESC selection.
pub fn decode_x3f_legacy(
    bytes: &[u8],
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::DecodedRaster, X3fError> {
    let inventory = inspect_x3f(bytes, &mut cancelled)?;
    let mut mode = [0u8; 64];
    let mut length = None;
    for entry in inventory.entries() {
        if let Some(properties) = entry.properties(&mut cancelled)? {
            for (key, value) in properties.entries() {
                if cancelled() { return Err(X3fError::Cancelled); }
                if key != b"W\0B\0_\0D\0E\0S\0C\0" { continue; }
                if length.is_some() || value.is_empty() || value.len() > 128 {
                    return Err(X3fError::Invalid);
                }
                for (index, pair) in value.chunks_exact(2).enumerate() {
                    let unit = u16::from_le_bytes(pair.try_into().unwrap());
                    if unit == 0 || unit > 127 { return Err(X3fError::Invalid); }
                    mode[index] = unit as u8;
                }
                length = Some(value.len()/2);
            }
        }
    }
    let length = length.ok_or(X3fError::Invalid)?;
    decode_x3f_legacy_with_mode(bytes, &mode[..length], budget, &mut cancelled)
}

/// Native legacy encoding-6 / CAMF-type-2 Auto decode using the qualified
/// Foveon linear-sRGB convention. Other X3F sensor generations fail explicitly.
pub fn decode_x3f_legacy_auto(
    bytes: &[u8],
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::DecodedRaster, X3fError> {
    decode_x3f_legacy_with_mode(bytes, b"Auto", budget, &mut cancelled)
}
fn decode_x3f_legacy_with_mode(
    bytes: &[u8],
    mode: &[u8],
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::DecodedRaster, X3fError> {
    let inventory = inspect_x3f(bytes, &mut cancelled)?;
    let mut sensor = None;
    let mut calibration = None;
    for entry in inventory.entries() {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if matches!(&entry.kind, b"IMAG" | b"IMA2") {
            if let Some(info) = entry.image_info()? {
                if info.image_type == 3 {
                    if sensor.replace(entry).is_some() {
                        return Err(X3fError::Invalid);
                    }
                }
            }
        }
        if entry.kind == *b"CAMF" {
            if calibration.replace(entry).is_some() {
                return Err(X3fError::Invalid);
            }
        }
    }
    let sensor = sensor.ok_or(X3fError::Invalid)?.legacy_huffman(&mut cancelled)?;
    let channels = sensor.decode_channels(budget, &mut cancelled)?;
    let calibration = calibration
        .ok_or(X3fError::Invalid)?
        .camf()?
        .ok_or(X3fError::Invalid)?
        .decode_type2(budget, &mut cancelled)?;
    let keep = unique_camf_matrix(&calibration, b"KeepImageArea", &mut cancelled)?;
    let active = unique_camf_matrix(&calibration, b"ActiveImageArea", &mut cancelled)?;
    if keep.element_type != 1 || active.element_type != 1 || keep.elements != 4 || active.elements != 4 {
        return Err(X3fError::Invalid);
    }
    let area = [
        word(active.data, 0),
        word(active.data, 4)
            .checked_sub(word(keep.data, 4))
            .ok_or(X3fError::Invalid)?,
        word(active.data, 8),
        word(active.data, 12).checked_sub(2).ok_or(X3fError::Invalid)?,
    ];
    let width = area[2].checked_sub(area[0]).ok_or(X3fError::Invalid)?;
    let height = area[3].checked_sub(area[1]).ok_or(X3fError::Invalid)?;
    let target = [
        [1.4032, -0.2231, -0.1016],
        [-0.5263, 1.4816, 0.017],
        [-0.0112, 0.0183, 0.9113],
    ];
    let frame = channels.process_with_white_balance(&calibration, mode, target, area, budget, &mut cancelled)?;
    drop(channels);
    drop(calibration);
    let output = x3f_linear_output(&frame, target, budget, &mut cancelled)?;
    drop(frame);
    if inventory.rotation_degrees == 0 {
        x3f_linear_raster(&output, width, height, budget, &mut cancelled)
    } else {
        let (rotated, w, h) = rotate_x3f_linear(
            &output,
            width,
            height,
            inventory.rotation_degrees,
            budget,
            &mut cancelled,
        )?;
        drop(output);
        x3f_linear_raster(&rotated, w, h, budget, &mut cancelled)
    }
}

/// Rotate RGB16 clockwise according to the X3F header's degree convention.
pub fn rotate_x3f_linear(
    rgb: &[u16],
    width: u32,
    height: u32,
    degrees: u32,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(rrrah_core::PixelBuffer<u16>, u32, u32), X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if width == 0
        || height == 0
        || rgb.len() as u64 != u64::from(width) * u64::from(height) * 3
        || !matches!(degrees, 0 | 90 | 180 | 270)
    {
        return Err(X3fError::Invalid);
    }
    let (w, h) = if degrees % 180 == 0 {
        (width, height)
    } else {
        (height, width)
    };
    let mut output = budget.try_buffer(rgb.len(), 0u16)?;
    for y in 0..height {
        for x in 0..width {
            if x % 256 == 0 && cancelled() {
                return Err(X3fError::Cancelled);
            }
            let (dx, dy) = match degrees {
                0 => (x, y),
                90 => (height - 1 - y, x),
                180 => (width - 1 - x, height - 1 - y),
                270 => (y, width - 1 - x),
                _ => unreachable!(),
            };
            let source = (y as usize * width as usize + x as usize) * 3;
            let dest = (dy as usize * w as usize + dx as usize) * 3;
            output[dest..dest + 3].copy_from_slice(&rgb[source..source + 3]);
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok((output.freeze().into(), w, h))
}

/// Managed legacy Foveon noise curve with a separately represented bounded domain.
#[derive(Debug)]
pub struct X3fNoiseCurve {
    samples: rrrah_core::PixelBuffer<i16>,
}
impl X3fNoiseCurve {
    pub fn build(
        maximum: f64,
        multiplier: f64,
        filter: f64,
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Self, X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if !maximum.is_finite()
            || !multiplier.is_finite()
            || !filter.is_finite()
            || maximum <= 0.0
            || multiplier <= 0.0
            || filter < 0.0
        {
            return Err(X3fError::Invalid);
        }
        let filter = if filter == 0.0 { 0.8 } else { filter };
        let length = 4.0 * std::f64::consts::PI * maximum / filter;
        if !length.is_finite() || length < 1.0 || length >= 32768.0 {
            return Err(X3fError::Limit);
        }
        let count = length as usize;
        let mut samples = budget.try_buffer(count, 0i16)?;
        for (i, value) in samples.iter_mut().enumerate() {
            if i % 256 == 0 && cancelled() {
                return Err(X3fError::Cancelled);
            }
            let x = i as f64 * filter / maximum / 4.0;
            let result = (x.cos() + 1.0) / 2.0 * (i as f64 * filter / multiplier).tanh() * multiplier + 0.5;
            if !result.is_finite() || !(0.0..32768.0).contains(&result) {
                return Err(X3fError::Limit);
            }
            *value = result as i16;
        }
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        Ok(Self {
            samples: samples.freeze().into(),
        })
    }
    pub fn domain(&self) -> usize {
        self.samples.len()
    }
    pub fn apply(&self, difference: i32) -> i32 {
        let magnitude = difference.unsigned_abs() as usize;
        let Some(value) = self.samples.get(magnitude) else {
            return 0;
        };
        if difference < 0 {
            -i32::from(*value)
        } else {
            i32::from(*value)
        }
    }
}

/// Copy an explicit half-open active rectangle into a managed channel buffer.
pub fn crop_x3f_channels(
    pixels: &[i32],
    width: u32,
    height: u32,
    area: [u32; 4],
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::PixelBuffer<i32>, X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    let [left, top, right, bottom] = area;
    if pixels.len() as u64 != u64::from(width) * u64::from(height) * 3
        || left >= right
        || top >= bottom
        || right > width
        || bottom > height
    {
        return Err(X3fError::Invalid);
    }
    let row = (right - left) as usize * 3;
    let count = row.checked_mul((bottom - top) as usize).ok_or(X3fError::Limit)?;
    let mut output = budget.try_buffer(count, 0i32)?;
    for y in top..bottom {
        let source = ((y as usize) * (width as usize) + left as usize) * 3;
        let dest = (y - top) as usize * row;
        for x in (0..row).step_by(768) {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            let end = (x + 768).min(row);
            output[dest + x..dest + end].copy_from_slice(&pixels[source + x..source + end]);
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(output.freeze().into())
}

/// Final legacy chroma pass with managed guide and three scratch rows.
/// Late cancellation invalidates the partially modified scratch frame.
pub fn smooth_x3f_chroma(
    pixels: &mut [i32],
    width: u32,
    height: u32,
    curves: [&X3fNoiseCurve; 3],
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), X3fError> {
    let guide = x3f_chroma_guide(pixels, width, height, budget, &mut cancelled)?;
    let w = width as usize;
    let active = w & !3;
    let rows = height as usize & !3;
    let stride = active.checked_mul(3).ok_or(X3fError::Limit)?;
    // Three managed rows: reverse horizontal, forward horizontal, vertical.
    let mut scratch = budget.try_buffer(stride.checked_mul(3).ok_or(X3fError::Limit)?, 0i64)?;
    for y in 0..rows {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if y % 4 == 0 {
            let mut previous = [0i64; 3];
            for x in (0..active).rev() {
                if x % 256 == 0 && cancelled() {
                    return Err(X3fError::Cancelled);
                }
                for c in 0..3 {
                    previous[c] = (i64::from(guide[((y / 4) * (w / 4) + x / 4) * 3 + c]) * 1485
                        + previous[c] * 6707
                        + 4096)
                        >> 13;
                    scratch[x * 3 + c] = previous[c];
                }
            }
        }
        let mut previous = [0i64; 3];
        for x in 0..active {
            if x % 256 == 0 && cancelled() {
                return Err(X3fError::Cancelled);
            }
            for c in 0..3 {
                let i = x * 3 + c;
                previous[c] = (scratch[i] * 1485 + previous[c] * 6707 + 4096) >> 13;
                scratch[stride + i] = previous[c];
                scratch[2 * stride + i] = if y == 0 {
                    previous[c]
                } else {
                    (scratch[2 * stride + i] * 6707 + previous[c] * 1485 + 4096) >> 13
                };
            }
        }
        for x in 0..active {
            if x % 256 == 0 && cancelled() {
                return Err(X3fError::Cancelled);
            }
            let index = (y * w + x) * 3;
            let smooth = &scratch[2 * stride + x * 3..2 * stride + x * 3 + 3];
            let denominator = 30 + smooth.iter().sum::<i64>();
            let signal = 30
                + pixels[index..index + 3]
                    .iter()
                    .map(|v| i64::from(*v))
                    .sum::<i64>();
            let ratio = (signal << 16) / denominator;
            let mut correction = [0i32; 3];
            for c in 0..3 {
                let target = ((smooth[c] * ratio + 32768) >> 16) - i64::from(pixels[index + c]);
                correction[c] = curves[c].apply(i32::try_from(target).map_err(|_| X3fError::Invalid)?);
            }
            let common = correction.iter().sum::<i32>() >> 3;
            for c in 0..3 {
                pixels[index + c] = (pixels[index + c] + correction[c] - common).max(0);
            }
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(())
}

/// Bottom-up quarter-resolution guide, excluding incomplete 4x4 blocks.
pub fn x3f_chroma_guide(
    pixels: &[i32],
    width: u32,
    height: u32,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::PixelBuffer<i16>, X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if width < 4 || height < 4 || pixels.len() as u64 != u64::from(width) * u64::from(height) * 3 {
        return Err(X3fError::Invalid);
    }
    for chunk in pixels.chunks(768) {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if chunk.iter().any(|v| !(0..=24000).contains(v)) {
            return Err(X3fError::Invalid);
        }
    }
    let w = width as usize;
    let qw = w / 4;
    let qh = height as usize / 4;
    let count = qw
        .checked_mul(qh)
        .and_then(|v| v.checked_mul(3))
        .ok_or(X3fError::Limit)?;
    let mut guide = budget.try_buffer(count, 0i16)?;
    for y in (0..qh).rev() {
        for x in 0..qw {
            if x % 256 == 0 && cancelled() {
                return Err(X3fError::Cancelled);
            }
            let mut sum = [0i64; 3];
            for dy in 0..4 {
                for dx in 0..4 {
                    for c in 0..3 {
                        sum[c] += i64::from(pixels[((y * 4 + dy) * w + x * 4 + dx) * 3 + c]);
                    }
                }
            }
            for c in 0..3 {
                let value = if y + 1 == qh {
                    sum[c] >> 4
                } else {
                    (i64::from(guide[((y + 1) * qw + x) * 3 + c]) * 1840 + sum[c] * 141 + 2048) >> 12
                };
                guide[(y * qw + x) * 3 + c] = i16::try_from(value).map_err(|_| X3fError::Invalid)?;
            }
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(guide.freeze().into())
}

/// Derived legacy Foveon transform and the luminance used by its noise curves.
#[derive(Debug, Clone, Copy)]
pub struct X3fColorTransform {
    pub coefficients: [[f32; 3]; 3],
    pub luminance: f64,
}
/// Explicit final linear output matrix; no exposure adjustment or transfer encoding.
pub fn x3f_linear_output(
    channels: &[i32],
    matrix: [[f32; 3]; 3],
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::PixelBuffer<u16>, X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if channels.is_empty()
        || channels.len() % 3 != 0
        || matrix
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > f32::MAX / 131072.0)
    {
        return Err(X3fError::Invalid);
    }
    let mut output = budget.try_buffer(channels.len(), 0u16)?;
    for (i, pixel) in channels.chunks_exact(3).enumerate() {
        if i % 256 == 0 && cancelled() {
            return Err(X3fError::Cancelled);
        }
        if pixel.iter().any(|v| !(0..=65535).contains(v)) {
            return Err(X3fError::Invalid);
        }
        for c in 0..3 {
            let mut value = 0f32;
            for k in 0..3 {
                value += matrix[c][k] * pixel[k] as f32;
            }
            if !value.is_finite() {
                return Err(X3fError::Invalid);
            }
            output[i * 3 + c] = value.clamp(0.0, 65535.0) as u16;
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(output.freeze().into())
}
/// Pack already qualified linear-sRGB channels for the existing raster/GPU path.
pub fn x3f_linear_raster(
    rgb: &[u16],
    width: u32,
    height: u32,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::DecodedRaster, X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || rgb.len() as u64 != pixels * 3 {
        return Err(X3fError::Invalid);
    }
    let count =
        usize::try_from(pixels.checked_mul(4).ok_or(X3fError::Limit)?).map_err(|_| X3fError::Limit)?;
    let mut rgba = budget.try_buffer(count, 0u16)?;
    for (i, (source, target)) in rgb.chunks_exact(3).zip(rgba.chunks_exact_mut(4)).enumerate() {
        if i % 256 == 0 && cancelled() {
            return Err(X3fError::Cancelled);
        }
        target[..3].copy_from_slice(source);
        target[3] = 65535;
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    rrrah_core::DecodedRaster::new(
        width,
        height,
        rrrah_core::RasterPixels::Rgba16(rgba.freeze().into()),
        rrrah_core::RasterColorSpace::LinearSrgb,
    )
    .map_err(|_| X3fError::Invalid)
}
/// Apply explicit channel curves and transform in place. Late cancellation may
/// leave a transformed prefix; callers must discard that scratch frame.
pub fn transform_x3f_pixels(
    pixels: &mut [i32],
    curves: [&X3fNoiseCurve; 3],
    transform: &X3fColorTransform,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if pixels.is_empty()
        || pixels.len() % 3 != 0
        || transform
            .coefficients
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > f32::MAX / 131072.0)
    {
        return Err(X3fError::Invalid);
    }
    // Validate before modifying any pixels, with bounded cancellation latency.
    for chunk in pixels.chunks(768) {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if chunk
            .iter()
            .any(|v| !(i16::MIN as i32..=i16::MAX as i32).contains(v))
        {
            return Err(X3fError::Invalid);
        }
    }
    for (index, pixel) in pixels.chunks_exact_mut(3).enumerate() {
        if index % 256 == 0 && cancelled() {
            return Err(X3fError::Cancelled);
        }
        let mut channel = [pixel[0], pixel[1], pixel[2]];
        for c in 0..3 {
            channel[c] -= curves[c].apply(channel[c]);
        }
        let mean = (channel[0] + 2 * channel[1] + channel[2]) >> 2;
        for c in 0..3 {
            channel[c] -= curves[c].apply(channel[c] - mean);
        }
        for c in 0..3 {
            let mut sum = 0.0f64;
            for i in 0..3 {
                // Legacy multiplication is float32; accumulation is float64.
                sum += f64::from(transform.coefficients[c][i] * channel[i] as f32);
            }
            pixel[c] = (sum.clamp(0.0, 24000.0) + 0.5) as i32;
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(())
}
pub fn x3f_color_transform_auto(
    calibration: &[u8],
    target_from_xyz: [[f32; 3]; 3],
    mut cancelled: impl FnMut() -> bool,
) -> Result<X3fColorTransform, X3fError> {
    x3f_color_transform_for_mode(calibration, b"Auto", target_from_xyz, &mut cancelled)
}
fn x3f_color_transform_for_mode(
    calibration: &[u8],
    mode: &[u8],
    target_from_xyz: [[f32; 3]; 3],
    mut cancelled: impl FnMut() -> bool,
) -> Result<X3fColorTransform, X3fError> {
    if target_from_xyz.iter().flatten().any(|v| !v.is_finite()) {
        return Err(X3fError::Invalid);
    }
    let xyz = x3f_illuminant_matrix(calibration, mode, &mut cancelled)?;
    let correction = x3f_wb_correction(calibration, mode, &mut cancelled)?;
    let mut neutral = x3f_neutral_response(calibration, mode, &mut cancelled)?;
    let maximum = neutral.iter().copied().fold(0f32, f32::max);
    for v in &mut neutral {
        *v /= maximum;
    }
    let mut combined = [[0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for c in 0..3 {
                combined[i][j] += correction[i * 3 + c] * xyz[c * 3 + j];
            }
        }
    }
    let mut target = [[0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for c in 0..3 {
                target[i][j] += target_from_xyz[i][c] * combined[c][j] * neutral[j];
            }
        }
    }
    let mut sums = [0f64; 3];
    for i in 0..3 {
        sums[i] = f64::from(target[i][0] + target[i][1] + target[i][2]);
    }
    let luminance = (6.0 * sums[0] + 11.0 * sums[1] + 3.0 * sums[2]) / 20.0;
    if !luminance.is_finite() || luminance <= 0.0 || sums.iter().any(|v| !v.is_finite() || *v == 0.0) {
        return Err(X3fError::Invalid);
    }
    for i in 0..3 {
        for j in 0..3 {
            combined[i][j] = (f64::from(target[i][j]) * luminance / sums[i]) as f32;
        }
    }
    let mut coefficients = [[0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for c in 0..3 {
                coefficients[i][j] += (if i == c { 32.0 } else { -1.0 }) * combined[c][j] / 30.0;
            }
        }
    }
    if coefficients.iter().flatten().any(|v| !v.is_finite()) {
        return Err(X3fError::Invalid);
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(X3fColorTransform {
        coefficients,
        luminance,
    })
}

/// Eight managed legacy noise curves, using explicitly supplied transform luminance.
pub fn x3f_noise_curves_auto(
    calibration: &[u8],
    luminance: f64,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<[X3fNoiseCurve; 8], X3fError> {
    x3f_noise_curves_for_mode(calibration, b"Auto", luminance, budget, &mut cancelled)
}
fn x3f_noise_curves_for_mode(
    calibration: &[u8],
    mode: &[u8],
    luminance: f64,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<[X3fNoiseCurve; 8], X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if !luminance.is_finite() || luminance <= 0.0 { return Err(X3fError::Invalid); }
    let mut response = x3f_neutral_response(calibration, mode, &mut cancelled)?;
    let maximum = response.iter().copied().fold(0f32, f32::max);
    for v in &mut response {
        *v /= maximum;
    }
    let color_name = if x3f_block_enabled(calibration, b"ColorDQ", &mut cancelled)? {
        b"ColorDQ".as_slice()
    } else {
        b"ColorDQCamRGB".as_slice()
    };
    let color = unique_camf_matrix(calibration, color_name, &mut cancelled)?;
    let chroma = unique_camf_matrix(calibration, b"ChromaDQ", &mut cancelled)?;
    if color.elements != 3 || chroma.elements != 3 {
        return Err(X3fError::Invalid);
    }
    let filter = if x3f_block_enabled(calibration, b"ColumnFilter", &mut cancelled)? {
        let m = unique_camf_matrix(calibration, b"ColumnFilter", &mut cancelled)?;
        if m.elements != 1 {
            return Err(X3fError::Invalid);
        }
        f64::from(m.float32(0)?)
    } else {
        0.0
    };
    let mut color_mul = [0f64; 3];
    let mut chroma_mul = [0f64; 3];
    let mut combined = luminance;
    for c in 0..3 {
        color_mul[c] = f64::from(color.float32(c)? / response[c]);
        let dq = chroma.float32(c)? / 3.0;
        chroma_mul[c] = f64::from(dq / response[c]);
        combined += chroma_mul[c];
    }
    let color_max = color_mul.iter().copied().fold(0f64, f64::max);
    let chroma_max = chroma_mul.iter().copied().fold(0f64, f64::max);
    let mut build = |max, mul| X3fNoiseCurve::build(max, mul, filter, budget, &mut cancelled);
    Ok([
        build(color_max, color_mul[0])?,
        build(color_max, color_mul[1])?,
        build(color_max, color_mul[2])?,
        build(chroma_max, chroma_mul[0])?,
        build(chroma_max, chroma_mul[1])?,
        build(chroma_max, chroma_mul[2])?,
        build(combined, combined)?,
        build(combined * 2.0, combined * 2.0)?,
    ])
}

/// First legacy hue-noise pass with a supplied qualified noise curve.
pub fn smooth_x3f_hues(
    pixels: &mut [i32],
    width: u32,
    height: u32,
    curve: &X3fNoiseCurve,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if width < 5 || height < 5 || pixels.len() as u64 != u64::from(width) * u64::from(height) * 3 {
        return Err(X3fError::Invalid);
    }
    for chunk in pixels.chunks(768) {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if chunk
            .iter()
            .any(|v| !(i16::MIN as i32..=i16::MAX as i32).contains(v))
        {
            return Err(X3fError::Invalid);
        }
    }
    let w = width as usize;
    let h = height as usize;
    let mut ring = budget.try_buffer(w.checked_mul(15).ok_or(X3fError::Limit)?, 0i64)?;
    let mut loaded = 0usize;
    for row in 2..h - 2 {
        while loaded <= row + 2 {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            for col in 2..w - 2 {
                if col % 256 == 2 && cancelled() {
                    return Err(X3fError::Cancelled);
                }
                for c in 0..3 {
                    let at = |x: usize| i64::from(pixels[(loaded * w + x) * 3 + c]);
                    ring[((loaded % 5) * w + col) * 3 + c] =
                        (at(col - 1) + 2 * at(col) + at(col + 1) + 2) >> 2;
                }
            }
            loaded += 1;
        }
        for col in 2..w - 2 {
            if col % 256 == 2 && cancelled() {
                return Err(X3fError::Cancelled);
            }
            let index = (row * w + col) * 3;
            let mut deviation = [0i32; 3];
            for c in 0..3 {
                let at = |y: usize| ring[((y % 5) * w + col) * 3 + c];
                let average = (at(row - 1) + 2 * at(row) + at(row + 1)) >> 2;
                let difference =
                    i32::try_from(i64::from(pixels[index + c]) - average).map_err(|_| X3fError::Limit)?;
                deviation[c] = -curve.apply(difference);
            }
            let common = (deviation[0] + deviation[1] + deviation[2]) >> 3;
            for c in 0..3 {
                pixels[index + c] = pixels[index + c]
                    .checked_add(deviation[c] - common)
                    .ok_or(X3fError::Limit)?;
            }
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(())
}

/// Second legacy hue pass, with a 5x5 estimate and total-signal normalization.
pub fn smooth_x3f_hues_wide(
    pixels: &mut [i32],
    width: u32,
    height: u32,
    curve: &X3fNoiseCurve,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if width < 5 || height < 5 || pixels.len() as u64 != u64::from(width) * u64::from(height) * 3 {
        return Err(X3fError::Invalid);
    }
    for chunk in pixels.chunks(768) {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if chunk
            .iter()
            .any(|v| !(i16::MIN as i32..=i16::MAX as i32).contains(v))
        {
            return Err(X3fError::Invalid);
        }
    }
    let w = width as usize;
    let h = height as usize;
    let mut ring = budget.try_buffer(w.checked_mul(15).ok_or(X3fError::Limit)?, 0i64)?;
    let mut loaded = 0usize;
    for row in 2..h - 2 {
        while loaded <= row + 2 {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            for col in 2..w - 2 {
                if col % 256 == 2 && cancelled() {
                    return Err(X3fError::Cancelled);
                }
                for c in 0..3 {
                    let at = |x: usize| i64::from(pixels[(loaded * w + x) * 3 + c]);
                    ring[((loaded % 5) * w + col) * 3 + c] =
                        (at(col - 2) + at(col - 1) + at(col) + at(col + 1) + at(col + 2) + 2) >> 2;
                }
            }
            loaded += 1;
        }
        for col in 2..w - 2 {
            if col % 256 == 2 && cancelled() {
                return Err(X3fError::Cancelled);
            }
            let index = (row * w + col) * 3;
            let mut totals = [0i64; 3];
            let mut all = 375i64;
            let mut sum = 60i64;
            for c in 0..3 {
                for y in row - 2..=row + 2 {
                    totals[c] += ring[((y % 5) * w + col) * 3 + c];
                }
                all += totals[c];
                sum += i64::from(pixels[index + c]);
            }
            sum = sum.max(0);
            let ratio = if all > 375 { (sum << 16) / all } else { sum * 174 };
            for c in 0..3 {
                let target = (ratio.checked_mul(totals[c]).ok_or(X3fError::Limit)? + 32768) >> 16;
                let difference =
                    i32::try_from(target - i64::from(pixels[index + c])).map_err(|_| X3fError::Limit)?;
                pixels[index + c] = pixels[index + c]
                    .checked_add(curve.apply(difference))
                    .ok_or(X3fError::Limit)?;
            }
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(())
}

/// Resolves calibrated Auto neutral and u16 saturation calibration.
pub fn linearize_x3f_highlights_from_camf(
    pixels: &mut [i32],
    calibration: &[u8],
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), X3fError> {
    linearize_x3f_highlights_for_mode(pixels, calibration, b"Auto", &mut cancelled)
}
fn linearize_x3f_highlights_for_mode(
    pixels: &mut [i32],
    calibration: &[u8],
    mode: &[u8],
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), X3fError> {
    let neutral = x3f_neutral_response(calibration, mode, &mut cancelled)?;
    let saturation = unique_camf_matrix(calibration, b"SaturationLevel", &mut cancelled)?;
    if saturation.elements != 3 || saturation.element_type != 0 { return Err(X3fError::Invalid); }
    let mut levels = [0u32; 3];
    for (i, level) in levels.iter_mut().enumerate() {
        *level = u32::from(u16::from_le_bytes(
            saturation.data[i * 2..i * 2 + 2].try_into().unwrap(),
        ));
    }
    linearize_x3f_highlights(
        pixels,
        levels,
        neutral,
        &mut cancelled,
    )
}

/// Legacy highlight linearity adjustment using explicit saturation and neutral response.
pub fn linearize_x3f_highlights(
    pixels: &mut [i32],
    saturation: [u32; 3],
    neutral: [f32; 3],
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if pixels.len() % 3 != 0
        || saturation.iter().any(|v| *v == 0)
        || neutral.iter().any(|v| !v.is_finite() || *v <= 0.0)
    {
        return Err(X3fError::Invalid);
    }
    let maximum = neutral.iter().copied().fold(0f32, f32::max);
    let mut minimum = 65535i64;
    for c in 0..3 {
        let response = neutral[c] / maximum;
        let level = saturation[c] as f32 / response;
        if !level.is_finite() {
            return Err(X3fError::Invalid);
        }
        minimum = minimum.min(level as i64);
    }
    let limit = minimum * 9 >> 4;
    if limit == 0 {
        return Err(X3fError::Invalid);
    }
    for chunk in pixels.chunks(768) {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if chunk
            .iter()
            .any(|v| !(i16::MIN as i32..=i16::MAX as i32).contains(v))
        {
            return Err(X3fError::Invalid);
        }
    }
    for (index, pixel) in pixels.chunks_exact_mut(3).enumerate() {
        if index % 256 == 0 && cancelled() {
            return Err(X3fError::Cancelled);
        }
        if pixel.iter().any(|v| i64::from(*v) <= limit) {
            continue;
        }
        let low = i64::from(*pixel.iter().min().unwrap());
        let high = *pixel.iter().max().unwrap();
        if low >= limit * 2 {
            pixel.fill(high);
        } else {
            let inverse = 16384 - ((low - limit) << 14) / limit;
            let weight = 16384 - (inverse * inverse >> 14);
            let weight = weight * weight >> 14;
            for v in pixel {
                *v += ((i64::from(high - *v) * weight) >> 14) as i32;
            }
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(())
}

/// Legacy red sharpening against separable 5x5 Gaussian estimates.
/// Only interior red samples are changed; scratch is charged to MemoryBudget.
pub fn sharpen_x3f_red(
    pixels: &mut [i32],
    width: u32,
    height: u32,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if width < 5 || height < 5 || pixels.len() as u64 != u64::from(width) * u64::from(height) * 3 {
        return Err(X3fError::Invalid);
    }
    for chunk in pixels.chunks(768) {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if chunk
            .iter()
            .any(|v| !(i16::MIN as i32..=i16::MAX as i32).contains(v))
        {
            return Err(X3fError::Invalid);
        }
    }
    let w = width as usize;
    let h = height as usize;
    let mut ring = budget.try_buffer(w.checked_mul(5).ok_or(X3fError::Limit)?, 0i64)?;
    let mut loaded = 0usize;
    for row in 2..h - 2 {
        while loaded <= row + 2 {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            for col in 2..w - 2 {
                if col % 256 == 2 && cancelled() {
                    return Err(X3fError::Cancelled);
                }
                let at = |x: usize| i64::from(pixels[(loaded * w + x) * 3]);
                ring[(loaded % 5) * w + col] =
                    (at(col) * 6 + (at(col - 1) + at(col + 1)) * 4 + at(col - 2) + at(col + 2) + 8) >> 4;
            }
            loaded += 1;
        }
        let mut previous = 0i64;
        for col in 2..w - 2 {
            if col % 256 == 2 && cancelled() {
                return Err(X3fError::Cancelled);
            }
            let at = |y: usize| ring[(y % 5) * w + col];
            let smooth = (at(row) * 6 + (at(row - 1) + at(row + 1)) * 4 + at(row - 2) + at(row + 2) + 8) >> 4;
            if col == 2 {
                previous = smooth;
            }
            let index = (row * w + col) * 3;
            let red = i64::from(pixels[index]);
            let value = (red + ((red - ((smooth * 7 + previous) >> 3)) >> 3)).min(32000);
            pixels[index] = i32::try_from(value).map_err(|_| X3fError::Limit)?;
            previous = smooth;
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(())
}

/// Resolves legacy bad-pixel coordinates and codes from explicit CAMF matrices.
pub fn repair_x3f_bad_pixels_from_camf(
    pixels: &mut [i32],
    width: u32,
    height: u32,
    calibration: &[u8],
    mut cancelled: impl FnMut() -> bool,
) -> Result<usize, X3fError> {
    let keep = unique_camf_matrix(calibration, b"KeepImageArea", &mut cancelled)?;
    if keep.element_type != 1 || keep.elements != 4 {
        return Err(X3fError::Invalid);
    }
    let origin = [word(keep.data, 0), word(keep.data, 4)];
    let codes = unique_camf_matrix(calibration, b"BadPixels", &mut cancelled)?;
    if codes.element_type != 2 || codes.dimensions.len() != 12 {
        return Err(X3fError::Invalid);
    }
    repair_x3f_bad_pixels(pixels, width, height, origin, codes.data, &mut cancelled)
}

/// Repairs encoded legacy bad pixels in camera-channel scratch storage.
/// Coordinates outside the interior are skipped, as prescribed by the format path.
pub fn repair_x3f_bad_pixels(
    pixels: &mut [i32],
    width: u32,
    height: u32,
    origin: [u32; 2],
    codes: &[u8],
    mut cancelled: impl FnMut() -> bool,
) -> Result<usize, X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if width < 3
        || height < 3
        || pixels.len() as u64 != u64::from(width) * u64::from(height) * 3
        || codes.len() % 4 != 0
    {
        return Err(X3fError::Invalid);
    }
    let neighbors = [
        (-1i32, -1i32),
        (-1, 0),
        (-1, 1),
        (0, -1),
        (0, 1),
        (1, -1),
        (1, 0),
        (1, 1),
    ];
    let mut repaired = 0;
    for bytes in codes.chunks_exact(4) {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        let code = word(bytes, 0);
        let Some(x) = ((code >> 8) & 0xfff).checked_sub(origin[0]) else {
            continue;
        };
        let Some(y) = (code >> 20).checked_sub(origin[1]) else {
            continue;
        };
        if x == 0 || y == 0 || x >= width - 1 || y >= height - 1 {
            continue;
        }
        let mut sums = [0i64; 3];
        let mut count = 0;
        for (bit, (dy, dx)) in neighbors.iter().enumerate() {
            if code & (1 << bit) == 0 {
                continue;
            }
            let row = (i64::from(y) + i64::from(*dy)) as u64;
            let col = (i64::from(x) + i64::from(*dx)) as u64;
            let index = ((row * u64::from(width) + col) * 3) as usize;
            for c in 0..3 {
                let sample = pixels[index + c];
                if !(i16::MIN as i32..=i16::MAX as i32).contains(&sample) {
                    return Err(X3fError::Invalid);
                }
                sums[c] += i64::from(sample);
            }
            count += 1;
        }
        if count != 0 {
            let index = ((u64::from(y) * u64::from(width) + u64::from(x)) * 3) as usize;
            for c in 0..3 {
                pixels[index + c] = (sums[c] / count) as i32;
            }
            repaired += 1;
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(repaired)
}

/// Smooths interleaved per-row black estimates in place, before scene adjustment.
/// Cancellation can leave the caller's scratch buffer partially processed.
pub fn smooth_x3f_black_rows(
    rows: &mut [f32],
    mut cancelled: impl FnMut() -> bool,
) -> Result<[f32; 3], X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if rows.len() % 3 != 0 || rows.len() / 3 < 22 {
        return Err(X3fError::Invalid);
    }
    for chunk in rows.chunks(768) {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if chunk.iter().any(|v| !v.is_finite()) {
            return Err(X3fError::Invalid);
        }
    }
    let height = rows.len() / 3;
    rows.copy_within(24..48, 0);
    rows.copy_within((height - 22) * 3..(height - 11) * 3, (height - 11) * 3);
    let mut last = [[0f32; 3]; 3];
    for (i, row) in last.iter_mut().enumerate() {
        row.copy_from_slice(&rows[i * 3..i * 3 + 3]);
    }
    for row in 1..height - 1 {
        if row % 256 == 0 && cancelled() {
            return Err(X3fError::Cancelled);
        }
        for c in 0..3 {
            if last[1][c] > last[0][c] {
                if last[1][c] > last[2][c] {
                    rows[row * 3 + c] = last[0][c].max(last[2][c]);
                }
            } else if last[1][c] < last[2][c] {
                rows[row * 3 + c] = last[0][c].min(last[2][c]);
            }
        }
        last[0] = last[1];
        last[1] = last[2];
        last[2].copy_from_slice(&rows[(row + 1) * 3..(row + 2) * 3]);
    }
    for c in 0..3 {
        rows[(height - 1) * 3 + c] = (last[0][c] + last[1][c]) / 2.0;
        rows[c] = (rows[3 + c] + rows[9 + c]) / 2.0;
    }
    let alpha = (1.0 - (-1.0f64 / 24.0).exp()) as f32;
    let mut mean = [rows[0], rows[1], rows[2]];
    for row in 1..height {
        if row % 256 == 0 && cancelled() {
            return Err(X3fError::Cancelled);
        }
        for c in 0..3 {
            let previous = rows[(row - 1) * 3 + c];
            rows[row * 3 + c] = (rows[row * 3 + c] - previous) * alpha + previous;
            mean[c] += rows[row * 3 + c];
        }
    }
    let mut previous = [
        rows[(height - 1) * 3],
        rows[(height - 1) * 3 + 1],
        rows[(height - 1) * 3 + 2],
    ];
    for value in &mut mean {
        *value /= height as f32;
    }
    for row in (0..height).rev() {
        if row % 256 == 0 && cancelled() {
            return Err(X3fError::Cancelled);
        }
        for c in 0..3 {
            let value = (rows[row * 3 + c] - mean[c] - previous[c]) * alpha + previous[c];
            if !value.is_finite() {
                return Err(X3fError::Invalid);
            }
            rows[row * 3 + c] = value;
            previous[c] = value;
        }
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(mean)
}

#[derive(Clone, Copy)]
struct HuffNode {
    children: [usize; 2],
    value: Option<i16>,
}
const EMPTY_NODE: HuffNode = HuffNode {
    children: [usize::MAX; 2],
    value: None,
};
impl X3fLegacyHuffman<'_> {
    /// Decodes sensor channels only. No WB, color transform or preview fallback.
    pub fn decode_channels(
        &self,
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<X3fLegacyChannels, X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        let nodes = 1
            + (0..1024)
                .filter_map(|i| self.symbol(i))
                .map(|(_, length, _)| usize::from(length))
                .sum::<usize>();
        let mut tree = budget.try_buffer(nodes, EMPTY_NODE)?;
        let mut used = 1;
        for index in 0..1024 {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            let Some((difference, length, code)) = self.symbol(index) else {
                continue;
            };
            let mut node = 0;
            for bit in (0..length).rev() {
                let branch = ((code >> bit) & 1) as usize;
                if tree[node].children[branch] == usize::MAX {
                    tree[node].children[branch] = used;
                    used += 1;
                }
                node = tree[node].children[branch];
            }
            tree[node].value = Some(difference);
        }
        let count = usize::try_from(u64::from(self.info.width) * u64::from(self.info.height) * 3)
            .map_err(|_| X3fError::Invalid)?;
        let mut pixels = budget.try_buffer(count, 0u16)?;
        let mut offset = 0i16;
        let mut minimum = 0i16;
        {
            for row in 0..self.info.height as usize {
                if cancelled() {
                    return Err(X3fError::Cancelled);
                }
                let input = self.row(row).ok_or(X3fError::Invalid)?;
                let mut bit = 0usize;
                let mut predictor = [offset; 3];
                let width = self.info.width as usize;
                for column in 0..width {
                    if column % 256 == 0 && cancelled() {
                        return Err(X3fError::Cancelled);
                    }
                    for channel in 0..3 {
                        let mut node = 0;
                        let difference = loop {
                            if let Some(value) = tree[node].value {
                                break value;
                            }
                            let byte = *input.get(bit / 8).ok_or(X3fError::Invalid)?;
                            let branch = usize::from((byte >> (7 - bit % 8)) & 1);
                            bit += 1;
                            node = tree[node].children[branch];
                            if node == usize::MAX {
                                return Err(X3fError::Invalid);
                            }
                        };
                        predictor[channel] = predictor[channel].wrapping_add(difference);
                        minimum = minimum.min(predictor[channel]);
                        pixels[(row * width + column) * 3 + channel] = predictor[channel] as u16;
                    }
                }
            }
        }
        if minimum < 0 {
            offset = minimum.checked_neg().ok_or(X3fError::Invalid)?;
        }
        // Offset addition commutes with the row's wrapping predictor. Preserve
        // signed samples until this final scan; clamp only after adding offset.
        for chunk in pixels.chunks_mut(768) {
            if cancelled() { return Err(X3fError::Cancelled); }
            for value in chunk {
                *value = (*value as i16).wrapping_add(offset).max(0) as u16;
            }
        }
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        drop(tree);
        Ok(X3fLegacyChannels {
            width: self.info.width,
            height: self.info.height,
            legacy_offset: offset,
            pixels: pixels.freeze().into(),
        })
    }
    pub fn symbol(&self, index: usize) -> Option<(i16, u8, u32)> {
        if index >= 1024 {
            return None;
        }
        let code = word(self.codes, index * 4);
        if code == 0 {
            return None;
        }
        let difference = i16::from_le_bytes(self.mapping[index * 2..index * 2 + 2].try_into().unwrap());
        Some((difference, (code >> 27) as u8, code & 0x07ff_ffff))
    }
    pub fn row(&self, index: usize) -> Option<&[u8]> {
        if index >= self.info.height as usize {
            return None;
        }
        let start = word(self.offsets, index * 4) as usize;
        let end = if index + 1 == self.info.height as usize {
            self.stream.len()
        } else {
            word(self.offsets, (index + 1) * 4) as usize
        };
        Some(&self.stream[start..end])
    }
}
impl<'a> X3fEntry<'a> {
    /// Admits the legacy compressed SD10 layout; other sensor layouts refuse.
    pub fn legacy_huffman(
        &self,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<X3fLegacyHuffman<'a>, X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        let info = self.image_info()?.ok_or(X3fError::Invalid)?;
        if info.image_type != 3 || info.encoding != 6 || info.row_bytes != 0 {
            return Err(X3fError::Version);
        }
        let table_end = 28 + 1024 * 2 + 1024 * 4;
        let offsets_bytes = usize::try_from(u64::from(info.height) * 4).map_err(|_| X3fError::Invalid)?;
        let offsets_start = self
            .bytes
            .len()
            .checked_sub(offsets_bytes)
            .ok_or(X3fError::Invalid)?;
        if offsets_start <= table_end {
            return Err(X3fError::Invalid);
        }
        let layout = X3fLegacyHuffman {
            info,
            mapping: &self.bytes[28..2076],
            codes: &self.bytes[2076..table_end],
            stream: &self.bytes[table_end..offsets_start],
            offsets: &self.bytes[offsets_start..],
        };
        let mut symbols = 0;
        for index in 0..1024 {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            let Some((_, length, code)) = layout.symbol(index) else {
                continue;
            };
            if length == 0 || length > 27 || code >= (1u32 << length) {
                return Err(X3fError::Invalid);
            }
            symbols += 1;
            for prior in 0..index {
                if cancelled() {
                    return Err(X3fError::Cancelled);
                }
                if let Some((_, other_length, other_code)) = layout.symbol(prior) {
                    let common = length.min(other_length);
                    if code >> (length - common) == other_code >> (other_length - common) {
                        return Err(X3fError::Invalid);
                    }
                }
            }
        }
        if symbols == 0 {
            return Err(X3fError::Invalid);
        }
        let mut previous = None;
        for index in 0..info.height as usize {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            let offset = word(layout.offsets, index * 4) as usize;
            if offset >= layout.stream.len()
                || (index == 0 && offset != 0)
                || previous.is_some_and(|old| offset <= old)
            {
                return Err(X3fError::Invalid);
            }
            previous = Some(offset);
        }
        Ok(layout)
    }
}
/// Borrowed calibration header. Payload decoding is separate from display color.
#[derive(Debug, Clone, Copy)]
pub struct X3fCamf<'a> {
    pub encoding: u32,
    pub parameters: [u32; 4],
    pub payload: &'a [u8],
}
/// A bounded borrowed CAMF entry, before matrix/property interpretation.
#[derive(Debug, Clone, Copy)]
pub struct X3fCamfEntry<'a> {
    raw: &'a [u8],
    pub kind: [u8; 4],
    pub version: u32,
    pub name: &'a [u8],
    pub value: &'a [u8],
}
/// Bounded numerical payload, preserving source type and dimension order.
#[derive(Debug)]
pub struct X3fCamfMatrix<'a> {
    pub element_type: u32,
    pub dimensions: &'a [u8],
    pub data: &'a [u8],
    pub elements: usize,
}
impl X3fCamfMatrix<'_> {
    /// Linear top-to-bottom dark drift coefficients [channel][slope, offset].
    pub fn dark_drift(&self, row: u32, height: u32) -> Result<[[f32; 2]; 3], X3fError> {
        if self.element_type != 3
            || self.elements != 12
            || self.dimensions.len() != 36
            || word(self.dimensions, 0) != 2
            || word(self.dimensions, 12) != 3
            || word(self.dimensions, 24) != 2
            || height < 2
            || row >= height
        {
            return Err(X3fError::Invalid);
        }
        let position = f64::from(row) / f64::from(height - 1);
        let mut output = [[0f32; 2]; 3];
        for (channel, pair) in output.iter_mut().enumerate() {
            for (component, value) in pair.iter_mut().enumerate() {
                let top = self.float32(channel * 2 + component)?;
                let bottom = self.float32(6 + channel * 2 + component)?;
                *value = (f64::from(top) + position * f64::from(bottom - top)) as f32;
                if !value.is_finite() {
                    return Err(X3fError::Invalid);
                }
            }
        }
        Ok(output)
    }

    /// Evaluates the legacy fixed-point channel polynomial before gain/WB.
    pub fn post_polynomial(&self, input: [i32; 3]) -> Result<[i32; 3], X3fError> {
        if self.element_type != 3
            || self.elements != 27
            || self.dimensions.len() != 24
            || word(self.dimensions, 0) != 3
            || word(self.dimensions, 12) != 9
        {
            return Err(X3fError::Invalid);
        }
        let p = input.map(i64::from);
        let mut terms = [[0i64; 3]; 3];
        for c in 0..3 {
            terms[0][c] = p[c].checked_mul(p[c]).ok_or(X3fError::Limit)? >> 14;
            terms[2][c] = p[c].checked_mul(terms[0][c]).ok_or(X3fError::Limit)? >> 14;
            terms[1][2 - c] = p[(c + 1) % 3]
                .checked_mul(p[(c + 2) % 3])
                .ok_or(X3fError::Limit)?
                >> 14;
        }
        let mut output = [0i32; 3];
        for c in 0..3 {
            let mut correction = 0f32;
            for (group, term) in terms.iter().enumerate() {
                for (j, value) in term.iter().enumerate() {
                    correction += self.float32(c * 9 + group * 3 + j)? * *value as f32;
                }
            }
            let result = p[c] as f64 + f64::from(correction.floor());
            if !result.is_finite() || result < i32::MIN as f64 || result > i32::MAX as f64 {
                return Err(X3fError::Limit);
            }
            output[c] = result as i32;
        }
        Ok(output)
    }

    /// Bilinear gain at normalized grid coordinates, preserving CAMF storage order.
    /// SpatialGain stores row, column, channel dimensions; channel is fastest.
    pub fn spatial_gain(&self, x: f32, y: f32) -> Result<[f32; 3], X3fError> {
        if self.element_type != 3
            || self.dimensions.len() != 36
            || !x.is_finite()
            || !y.is_finite()
            || !(0.0..=1.0).contains(&x)
            || !(0.0..=1.0).contains(&y)
        {
            return Err(X3fError::Invalid);
        }
        let rows = word(self.dimensions, 0) as usize;
        let columns = word(self.dimensions, 12) as usize;
        if rows < 2 || columns < 2 || word(self.dimensions, 24) != 3 {
            return Err(X3fError::Invalid);
        }
        let expected = rows
            .checked_mul(columns)
            .and_then(|v| v.checked_mul(3))
            .ok_or(X3fError::Limit)?;
        if expected != self.elements {
            return Err(X3fError::Invalid);
        }
        let gx = x * (columns - 1) as f32;
        let gy = y * (rows - 1) as f32;
        let left = (gx.floor() as usize).min(columns - 2);
        let top = (gy.floor() as usize).min(rows - 2);
        let tx = gx - left as f32;
        let ty = gy - top as f32;
        let mut output = [0f32; 3];
        for (channel, value) in output.iter_mut().enumerate() {
            let a = self.float32((top * columns + left) * 3 + channel)?;
            let b = self.float32((top * columns + left + 1) * 3 + channel)?;
            let c = self.float32(((top + 1) * columns + left) * 3 + channel)?;
            let d = self.float32(((top + 1) * columns + left + 1) * 3 + channel)?;
            if [a, b, c, d].iter().any(|v| *v <= 0.0) {
                return Err(X3fError::Invalid);
            }
            *value = ((1.0 - tx) * a + tx * b) * (1.0 - ty) + ((1.0 - tx) * c + tx * d) * ty;
            if !value.is_finite() || *value <= 0.0 {
                return Err(X3fError::Invalid);
            }
        }
        Ok(output)
    }

    /// Reads a finite source float without silently reinterpreting integer matrices.
    pub fn float32(&self, index: usize) -> Result<f32, X3fError> {
        if self.element_type != 3 || index >= self.elements {
            return Err(X3fError::Invalid);
        }
        let start = index.checked_mul(4).ok_or(X3fError::Limit)?;
        let end = start.checked_add(4).ok_or(X3fError::Limit)?;
        let bytes = self.data.get(start..end).ok_or(X3fError::Invalid)?;
        let value = f32::from_le_bytes(bytes.try_into().unwrap());
        if !value.is_finite() {
            return Err(X3fError::Invalid);
        }
        Ok(value)
    }
    /// Axis sizes in logical order; on-disk dimension order may be permuted.
    pub fn axis_size(&self, axis: u32) -> Option<u32> {
        self.dimensions
            .chunks_exact(12)
            .find(|d| word(d, 8) == axis)
            .map(|d| word(d, 0))
    }
}

impl<'a> X3fCamfEntry<'a> {
    /// Visits calibration key/value links with bounded NUL-terminated byte strings.
    pub fn visit_properties(
        &self,
        mut cancelled: impl FnMut() -> bool,
        mut visitor: impl FnMut(&'a [u8], &'a [u8]),
    ) -> Result<bool, X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if &self.kind != b"CMbP" {
            return Ok(false);
        }
        let v = self.value;
        if v.len() < 8 {
            return Err(X3fError::Invalid);
        }
        let count = word(v, 0) as usize;
        if count > 1024 {
            return Err(X3fError::Limit);
        }
        let table = v.get(8..8 + count * 8).ok_or(X3fError::Invalid)?;
        let base = word(v, 4) as usize;
        if base < self.raw.len() - v.len() + 8 + count * 8 || base > self.raw.len() {
            return Err(X3fError::Invalid);
        }
        let string = |offset: u32| -> Result<&'a [u8], X3fError> {
            let start = base.checked_add(offset as usize).ok_or(X3fError::Limit)?;
            let tail = self.raw.get(start..).ok_or(X3fError::Invalid)?;
            let length = tail.iter().position(|&b| b == 0).ok_or(X3fError::Invalid)?;
            Ok(&tail[..length])
        };
        for pair in table.chunks_exact(8) {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            visitor(string(word(pair, 0))?, string(word(pair, 4))?);
        }
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        Ok(true)
    }

    pub fn matrix(&self) -> Result<Option<X3fCamfMatrix<'a>>, X3fError> {
        if &self.kind != b"CMbM" {
            return Ok(None);
        }
        let v = self.value;
        if v.len() < 12 {
            return Err(X3fError::Invalid);
        }
        let element_type = word(v, 0);
        let width = match element_type {
            0 | 6 => 2,
            1 | 2 | 3 => 4,
            5 => 1,
            _ => return Err(X3fError::Invalid),
        };
        let count = word(v, 4) as usize;
        if count == 0 || count > 8 {
            return Err(X3fError::Limit);
        }
        let dimensions = v.get(12..12 + count * 12).ok_or(X3fError::Invalid)?;
        let mut elements = 1usize;
        let mut axes = 0u32;
        for d in dimensions.chunks_exact(12) {
            let size = word(d, 0) as usize;
            let axis = word(d, 8);
            if size == 0 || axis as usize >= count || axes & (1 << axis) != 0 {
                return Err(X3fError::Invalid);
            }
            axes |= 1 << axis;
            elements = elements.checked_mul(size).ok_or(X3fError::Limit)?;
            let name_offset = word(d, 4) as usize;
            if name_offset < 20 {
                return Err(X3fError::Invalid);
            }
            let name = self.raw.get(name_offset..).ok_or(X3fError::Invalid)?;
            if !name.contains(&0) {
                return Err(X3fError::Invalid);
            }
        }
        let start = word(v, 8) as usize;
        let value_start = self.raw.len() - v.len();
        if start < value_start + 12 + count * 12 {
            return Err(X3fError::Invalid);
        }
        let end = start
            .checked_add(elements.checked_mul(width).ok_or(X3fError::Limit)?)
            .ok_or(X3fError::Limit)?;
        let data = self.raw.get(start..end).ok_or(X3fError::Invalid)?;
        Ok(Some(X3fCamfMatrix {
            element_type,
            dimensions,
            data,
            elements,
        }))
    }
}

fn unique_camf_matrix<'a>(
    bytes: &'a [u8],
    name: &[u8],
    cancelled: impl FnMut() -> bool,
) -> Result<X3fCamfMatrix<'a>, X3fError> {
    let mut count = 0;
    let mut matrix = None;
    let mut error = None;
    visit_x3f_camf(bytes, cancelled, |entry| {
        if entry.name == name {
            count += 1;
            match entry.matrix() {
                Ok(Some(m)) => matrix = Some(m),
                Ok(None) => error = Some(X3fError::Invalid),
                Err(e) => error = Some(e),
            }
        }
    })?;
    if let Some(e) = error {
        return Err(e);
    }
    if count != 1 {
        return Err(X3fError::Invalid);
    }
    matrix.ok_or(X3fError::Invalid)
}

/// IncludeBlocks is a presence table: an empty value still enables the block.
/// Ambiguous tables/duplicate keys and malformed entries are rejected.
pub fn x3f_block_enabled(
    bytes: &[u8],
    block: &[u8],
    mut cancelled: impl FnMut() -> bool,
) -> Result<bool, X3fError> {
    let mut tables = 0;
    let mut matches = 0;
    let mut error = None;
    visit_x3f_camf(bytes, &mut cancelled, |entry| {
        if entry.name == b"IncludeBlocks" {
            tables += 1;
            match entry.visit_properties(
                || false,
                |key, _| {
                    if key == block {
                        matches += 1;
                    }
                },
            ) {
                Ok(true) => (),
                Ok(false) => error = Some(X3fError::Invalid),
                Err(e) => error = Some(e),
            }
        }
    })?;
    if let Some(e) = error {
        return Err(e);
    }
    if tables != 1 || matches > 1 {
        return Err(X3fError::Invalid);
    }
    Ok(matches == 1)
}

/// Resolves the file's explicit WB correction link to a finite 3x3 matrix.
/// This is calibration extraction, not the full Foveon color pipeline.
pub fn x3f_wb_correction(
    bytes: &[u8],
    mode: &[u8],
    mut cancelled: impl FnMut() -> bool,
) -> Result<[f32; 9], X3fError> {
    x3f_linked_color_matrix(bytes, b"WhiteBalanceCorrections", mode, &mut cancelled)
}
/// Resolve an explicit WB mode from embedded neutral or calibrated camera matrices.
/// Returns unnormalized camera responses; never substitutes unity or identity.
pub fn x3f_neutral_response(
    bytes: &[u8],
    mode: &[u8],
    mut cancelled: impl FnMut() -> bool,
) -> Result<[f32; 3], X3fError> {
    if cancelled() { return Err(X3fError::Cancelled); }
    if mode.is_empty() || mode.len() > 64 || mode.contains(&0) {
        return Err(X3fError::Invalid);
    }
    let mut name = [0u8; 74];
    name[..mode.len()].copy_from_slice(mode);
    name[mode.len()..mode.len()+10].copy_from_slice(b"RGBNeutral");
    let name = &name[..mode.len()+10];
    let response = if x3f_block_enabled(bytes, name, &mut cancelled)? {
        let m = unique_camf_matrix(bytes, name, &mut cancelled)?;
        if m.elements != 3 { return Err(X3fError::Invalid); }
        [m.float32(0)?, m.float32(1)?, m.float32(2)?]
    } else {
        let xyz = x3f_illuminant_matrix(bytes, mode, &mut cancelled)?;
        let correction = x3f_wb_correction(bytes, mode, &mut cancelled)?;
        let mut combined = [[0f32; 3]; 3];
        for i in 0..3 { for j in 0..3 { for c in 0..3 {
            combined[i][j] += correction[i*3+c] * xyz[c*3+j];
        } } }
        let mut cofactors = [[0f32; 3]; 3];
        for i in 0..3 { for c in 0..3 {
            cofactors[c][i] = combined[(i+1)%3][(c+1)%3] * combined[(i+2)%3][(c+2)%3]
                - combined[(i+1)%3][(c+2)%3] * combined[(i+2)%3][(c+1)%3];
        } }
        let determinant = (0..3).map(|c| f64::from(combined[0][c]) * f64::from(cofactors[c][0])).sum::<f64>();
        if !determinant.is_finite() || determinant == 0.0 { return Err(X3fError::Invalid); }
        cofactors.map(|row| (f64::from(row[0])*0.3127 + f64::from(row[1])*0.329 + f64::from(row[2])*0.3583) as f32)
    };
    if response.iter().any(|v| !v.is_finite() || *v <= 0.0) { return Err(X3fError::Invalid); }
    if cancelled() { return Err(X3fError::Cancelled); }
    Ok(response)
}

/// Extracts the explicitly linked camera-to-XYZ illuminant coefficients.
/// Sensor corrections and matrix-axis interpretation remain separate.
pub fn x3f_illuminant_matrix(
    bytes: &[u8],
    mode: &[u8],
    mut cancelled: impl FnMut() -> bool,
) -> Result<[f32; 9], X3fError> {
    x3f_linked_color_matrix(bytes, b"WhiteBalanceIlluminants", mode, &mut cancelled)
}
fn x3f_linked_color_matrix(
    bytes: &[u8],
    table_name: &[u8],
    mode: &[u8],
    mut cancelled: impl FnMut() -> bool,
) -> Result<[f32; 9], X3fError> {
    let mut target = None;
    let mut tables = 0;
    let mut links = 0;
    let mut error = None;
    visit_x3f_camf(bytes, &mut cancelled, |entry| {
        if entry.name == table_name {
            tables += 1;
            match entry.visit_properties(
                || false,
                |key, value| {
                    if key == mode {
                        links += 1;
                        target = Some(value);
                    }
                },
            ) {
                Ok(true) => (),
                Ok(false) => error = Some(X3fError::Invalid),
                Err(e) => error = Some(e),
            }
        }
    })?;
    if let Some(e) = error {
        return Err(e);
    }
    if tables != 1 || links != 1 {
        return Err(X3fError::Invalid);
    }
    let target = target.ok_or(X3fError::Invalid)?;
    let mut found = 0;
    let mut output = [0f32; 9];
    visit_x3f_camf(bytes, &mut cancelled, |entry| {
        if entry.name != target {
            return;
        }
        found += 1;
        let read = || -> Result<[f32; 9], X3fError> {
            let m = entry.matrix()?.ok_or(X3fError::Invalid)?;
            if m.elements != 9
                || m.dimensions.len() != 24
                || m.axis_size(0) != Some(3)
                || m.axis_size(1) != Some(3)
            {
                return Err(X3fError::Invalid);
            }
            let mut result = [0f32; 9];
            for (i, value) in result.iter_mut().enumerate() {
                *value = m.float32(i)?;
            }
            Ok(result)
        };
        match read() {
            Ok(v) => output = v,
            Err(e) => error = Some(e),
        }
    })?;
    if let Some(e) = error {
        return Err(e);
    }
    if found != 1 {
        return Err(X3fError::Invalid);
    }
    Ok(output)
}

/// Visits calibration records without allocating an index or copying payloads.
pub fn visit_x3f_camf<'a>(
    bytes: &'a [u8],
    mut cancelled: impl FnMut() -> bool,
    mut visitor: impl FnMut(X3fCamfEntry<'a>),
) -> Result<(), X3fError> {
    let mut remaining = bytes;
    let mut count = 0;
    while !remaining.is_empty() {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        count += 1;
        if count > 1024 {
            return Err(X3fError::Limit);
        }
        if remaining.len() < 20 {
            return Err(X3fError::Invalid);
        }
        let kind: [u8; 4] = remaining[..4].try_into().unwrap();
        if !matches!(&kind, b"CMbP" | b"CMbT" | b"CMbM") {
            return Err(X3fError::Invalid);
        }
        let size = word(remaining, 8) as usize;
        if size < 20 {
            return Err(X3fError::Invalid);
        }
        let entry = remaining.get(..size).ok_or(X3fError::Invalid)?;
        let name_offset = word(entry, 12) as usize;
        let value_offset = word(entry, 16) as usize;
        if name_offset < 20 || value_offset < 20 {
            return Err(X3fError::Invalid);
        }
        let name = entry.get(name_offset..).ok_or(X3fError::Invalid)?;
        let name = &name[..name.iter().position(|&b| b == 0).ok_or(X3fError::Invalid)?];
        let value = entry.get(value_offset..).ok_or(X3fError::Invalid)?;
        visitor(X3fCamfEntry {
            raw: entry,
            kind,
            version: word(entry, 4),
            name,
            value,
        });
        remaining = &remaining[size..];
    }
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    Ok(())
}

impl X3fCamf<'_> {
    /// Deobfuscates legacy calibration bytes; does not apply camera color.
    pub fn decode_type2(
        &self,
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<u8>, X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if self.encoding != 2 || self.payload.is_empty() {
            return Err(X3fError::Invalid);
        }
        let mut output = budget.try_buffer(self.payload.len(), 0u8)?;
        let mut state = self.parameters[3];
        for (i, (&input, result)) in self.payload.iter().zip(output.iter_mut()).enumerate() {
            if i % 1024 == 0 && cancelled() {
                return Err(X3fError::Cancelled);
            }
            state = state.wrapping_mul(1597).wrapping_add(51749) % 244944;
            let quotient = ((u64::from(state) * 301593171) >> 24) as u32;
            let mask = ((state << 8).wrapping_sub(quotient) >> 1).wrapping_add(quotient) >> 17;
            *result = input ^ mask as u8;
        }
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        Ok(output.freeze().into())
    }
}
impl<'a> X3fEntry<'a> {
    pub fn camf(&self) -> Result<Option<X3fCamf<'a>>, X3fError> {
        if &self.kind != b"CAMF" {
            return Ok(None);
        }
        let b = self.bytes;
        if b.len() <= 28 || &b[..4] != b"SECc" {
            return Err(X3fError::Invalid);
        }
        if word(b, 4) != 0x20000 {
            return Err(X3fError::Version);
        }
        Ok(Some(X3fCamf {
            encoding: word(b, 8),
            parameters: [word(b, 12), word(b, 16), word(b, 20), word(b, 24)],
            payload: &b[28..],
        }))
    }
}

/// Validated UTF-16LE property strings, borrowed from the container.
#[derive(Debug)]
pub struct X3fProperties<'a> {
    table: &'a [u8],
    strings: &'a [u8],
}
impl<'a> X3fProperties<'a> {
    pub fn entries(&self) -> impl ExactSizeIterator<Item = (&'a [u8], &'a [u8])> + '_ {
        self.table.chunks_exact(8).map(|pair| {
            (
                property_string(self.strings, word(pair, 0) as usize).unwrap(),
                property_string(self.strings, word(pair, 4) as usize).unwrap(),
            )
        })
    }
}
fn property_string(strings: &[u8], offset: usize) -> Result<&[u8], X3fError> {
    let tail = strings
        .get(offset.checked_mul(2).ok_or(X3fError::Invalid)?..)
        .ok_or(X3fError::Invalid)?;
    let end = tail
        .chunks_exact(2)
        .position(|c| c == [0, 0])
        .ok_or(X3fError::Invalid)?
        * 2;
    let text = &tail[..end];
    if char::decode_utf16(text.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]))).any(|c| c.is_err())
    {
        return Err(X3fError::Invalid);
    }
    Ok(text)
}
impl<'a> X3fEntry<'a> {
    /// Validates property offsets, string termination and Unicode without allocation.
    pub fn properties(
        &self,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Option<X3fProperties<'a>>, X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        if &self.kind != b"PROP" {
            return Ok(None);
        }
        let b = self.bytes;
        if b.len() < 24 || &b[..4] != b"SECp" {
            return Err(X3fError::Invalid);
        }
        if word(b, 4) != 0x20000 {
            return Err(X3fError::Version);
        }
        let count = word(b, 8) as usize;
        if count > 1024 {
            return Err(X3fError::Limit);
        }
        if word(b, 12) != 0 || word(b, 16) != 0 {
            return Err(X3fError::Invalid);
        }
        let end = 24 + count * 8;
        let table = b.get(24..end).ok_or(X3fError::Invalid)?;
        let payload = b.get(end..).ok_or(X3fError::Invalid)?;
        let string_bytes = (word(b, 20) as usize).checked_mul(2).ok_or(X3fError::Invalid)?;
        let strings = payload.get(..string_bytes).ok_or(X3fError::Invalid)?;
        let padding = &payload[string_bytes..];
        if padding.len() > 3 || padding.iter().any(|&byte| byte != 0) {
            return Err(X3fError::Invalid);
        }
        for pair in table.chunks_exact(8) {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            property_string(strings, word(pair, 0) as usize)?;
            property_string(strings, word(pair, 4) as usize)?;
        }
        Ok(Some(X3fProperties { table, strings }))
    }
}

impl X3fEntry<'_> {
    /// Reports the image header without interpreting sensor compression/color.
    /// Type 2 denotes a processed preview, not the sensor image.
    pub fn image_info(&self) -> Result<Option<X3fImageInfo>, X3fError> {
        if !matches!(&self.kind, b"IMAG" | b"IMA2") {
            return Ok(None);
        }
        if self.bytes.len() < 28 || &self.bytes[..4] != b"SECi" {
            return Err(X3fError::Invalid);
        }
        if word(self.bytes, 4) != 0x0002_0000 {
            return Err(X3fError::Version);
        }
        let info = X3fImageInfo {
            image_type: word(self.bytes, 8),
            encoding: word(self.bytes, 12),
            width: word(self.bytes, 16),
            height: word(self.bytes, 20),
            row_bytes: word(self.bytes, 24),
        };
        if info.width == 0
            || info.height == 0
            || u64::from(info.width) * u64::from(info.height) > 100_000_000
            || info.row_bytes % 4 != 0
        {
            return Err(X3fError::Invalid);
        }
        Ok(Some(info))
    }
}

#[derive(Debug)]
pub struct X3fInventory<'a> {
    pub version: u32,
    /// Expected unrotated output dimensions, not sensor storage dimensions.
    pub width: u32,
    pub height: u32,
    pub rotation_degrees: u32,
    source: &'a [u8],
    directory: &'a [u8],
}
impl<'a> X3fInventory<'a> {
    pub fn entries(&self) -> impl ExactSizeIterator<Item = X3fEntry<'a>> + '_ {
        self.directory.chunks_exact(12).map(|record| {
            let offset = word(record, 0);
            let length = word(record, 4) as usize;
            X3fEntry {
                kind: record[8..12].try_into().unwrap(),
                offset,
                bytes: &self.source[offset as usize..offset as usize + length],
            }
        })
    }
}
fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

/// Validates ranges and overlap without allocation, copying or decoding pixels.
/// Unknown section types are retained. Property/color semantics are unqualified.
pub fn inspect_x3f(bytes: &[u8], mut cancelled: impl FnMut() -> bool) -> Result<X3fInventory<'_>, X3fError> {
    if cancelled() {
        return Err(X3fError::Cancelled);
    }
    if bytes.len() < 44 || &bytes[..4] != b"FOVb" {
        return Err(X3fError::Invalid);
    }
    let version = word(bytes, 4);
    if !(0x0002_0000..=0x0002_0002).contains(&version) {
        return Err(X3fError::Version);
    }
    let header_bytes = if version == 0x0002_0000 { 40 } else { 232 };
    let directory_offset = word(bytes, bytes.len() - 4) as usize;
    if directory_offset < header_bytes || directory_offset % 4 != 0 {
        return Err(X3fError::Invalid);
    }
    let directory = bytes
        .get(directory_offset..bytes.len() - 4)
        .ok_or(X3fError::Invalid)?;
    if directory.len() < 12 || &directory[..4] != b"SECd" || word(directory, 4) != 0x0002_0000 {
        return Err(X3fError::Invalid);
    }
    let count = word(directory, 8) as usize;
    if count > 1024 {
        return Err(X3fError::Limit);
    }
    if directory.len() != 12 + count * 12 {
        return Err(X3fError::Invalid);
    }
    let width = word(bytes, 28);
    let height = word(bytes, 32);
    let rotation_degrees = word(bytes, 36);
    if width == 0
        || height == 0
        || u64::from(width) * u64::from(height) > 100_000_000
        || !matches!(rotation_degrees, 0 | 90 | 180 | 270)
    {
        return Err(X3fError::Invalid);
    }
    let records = &directory[12..];
    for (index, record) in records.chunks_exact(12).enumerate() {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        let start = u64::from(word(record, 0));
        let end = start + u64::from(word(record, 4));
        if start < header_bytes as u64 || start % 4 != 0 || end <= start || end > directory_offset as u64 {
            return Err(X3fError::Invalid);
        }
        for prior in records[..index * 12].chunks_exact(12) {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            let prior_start = u64::from(word(prior, 0));
            let prior_end = prior_start + u64::from(word(prior, 4));
            if start < prior_end && prior_start < end {
                return Err(X3fError::Invalid);
            }
        }
    }
    Ok(X3fInventory {
        version,
        width,
        height,
        rotation_degrees,
        source: bytes,
        directory: records,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn decode_two_pass_reference(
        layout: &X3fLegacyHuffman<'_>,
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<X3fLegacyChannels, X3fError> {
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        let nodes = 1
            + (0..1024)
                .filter_map(|i| layout.symbol(i))
                .map(|(_, length, _)| usize::from(length))
                .sum::<usize>();
        let mut tree = budget.try_buffer(nodes, EMPTY_NODE)?;
        let mut used = 1;
        for index in 0..1024 {
            if cancelled() {
                return Err(X3fError::Cancelled);
            }
            let Some((difference, length, code)) = layout.symbol(index) else {
                continue;
            };
            let mut node = 0;
            for bit in (0..length).rev() {
                let branch = ((code >> bit) & 1) as usize;
                if tree[node].children[branch] == usize::MAX {
                    tree[node].children[branch] = used;
                    used += 1;
                }
                node = tree[node].children[branch];
            }
            tree[node].value = Some(difference);
        }
        let count = usize::try_from(u64::from(layout.info.width) * u64::from(layout.info.height) * 3)
            .map_err(|_| X3fError::Invalid)?;
        let mut pixels = budget.try_buffer(count, 0u16)?;
        let mut offset = 0i16;
        for pass in 0..2 {
            let mut minimum = 0i16;
            for row in 0..layout.info.height as usize {
                if cancelled() {
                    return Err(X3fError::Cancelled);
                }
                let input = layout.row(row).ok_or(X3fError::Invalid)?;
                let mut bit = 0usize;
                let mut predictor = [offset; 3];
                let width = layout.info.width as usize;
                for column in 0..width {
                    if column % 256 == 0 && cancelled() {
                        return Err(X3fError::Cancelled);
                    }
                    for channel in 0..3 {
                        let mut node = 0;
                        let difference = loop {
                            if let Some(value) = tree[node].value {
                                break value;
                            }
                            let byte = *input.get(bit / 8).ok_or(X3fError::Invalid)?;
                            let branch = usize::from((byte >> (7 - bit % 8)) & 1);
                            bit += 1;
                            node = tree[node].children[branch];
                            if node == usize::MAX {
                                return Err(X3fError::Invalid);
                            }
                        };
                        predictor[channel] = predictor[channel].wrapping_add(difference);
                        minimum = minimum.min(predictor[channel]);
                        pixels[(row * width + column) * 3 + channel] = predictor[channel].max(0) as u16;
                    }
                }
            }
            if pass == 0 && minimum < 0 {
                offset = minimum.checked_neg().ok_or(X3fError::Invalid)?;
            } else {
                break;
            }
        }
        if cancelled() {
            return Err(X3fError::Cancelled);
        }
        drop(tree);
        Ok(X3fLegacyChannels {
            width: layout.info.width,
            height: layout.info.height,
            legacy_offset: offset,
            pixels: pixels.freeze().into(),
        })
    }
    #[test]
    #[ignore = "requires pinned CC0 Sigma SD14 source and independent enabled LibRaw unpack"]
    fn sigma_sd14_native_sensor_matches_full_independent_unpack() {
        let source = std::fs::read(std::env::var_os("RRRAH_X3F_SD14_SOURCE").unwrap()).unwrap();
        let oracle = std::fs::read(std::env::var_os("RRRAH_X3F_SD14_ORACLE").unwrap()).unwrap();
        let inventory = inspect_x3f(&source, || false).unwrap();
        let sensor = inventory.entries().find(|entry| {
            entry.image_info().unwrap().is_some_and(|info| info.image_type == 3)
        }).unwrap();
        if std::env::var_os("RRRAH_X3F_SD14_SENSOR_TIMING").is_some() {
            let layout = sensor.legacy_huffman(|| false).unwrap();
            let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
            let mut times = [Vec::new(), Vec::new()];
            for iteration in 0..11 {
                for algorithm in if iteration % 2 == 0 { [0,1] } else { [1,0] } {
                    let start = std::time::Instant::now();
                    let result = if algorithm == 0 {
                        decode_two_pass_reference(&layout, &budget, || false)
                    } else { layout.decode_channels(&budget, || false) }.unwrap();
                    let elapsed = start.elapsed().as_secs_f64()*1000.0;
                    assert_eq!(oracle.len(), result.pixels.len()*2);
                    for (value, expected) in result.pixels.iter().zip(oracle.chunks_exact(2)) {
                        assert_eq!(*value,u16::from_le_bytes(expected.try_into().unwrap()));
                    }
                    drop(result); assert_eq!(budget.used(),0);
                    if iteration >= 2 { times[algorithm].push(elapsed); }
                }
            }
            for (name, mut samples) in ["two-pass", "one-pass"].into_iter().zip(times) {
                samples.sort_by(f64::total_cmp);
                eprintln!("SD14 sensor {name} p50={:.3} p95={:.3} ms n={} samples={samples:?}", samples[4], samples[8], samples.len());
            }
        }
        let budget = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
        let channels = sensor.legacy_huffman(|| false).unwrap().decode_channels(&budget, || false).unwrap();
        assert_eq!((channels.width, channels.height), (2688, 1792));
        assert_eq!(oracle.len(), channels.pixels.len() * 2);
        for (index, (actual, expected)) in channels.pixels.iter().zip(oracle.chunks_exact(2)).enumerate() {
            assert_eq!(*actual, u16::from_le_bytes(expected.try_into().unwrap()), "SD14 sensor sample {index}");
        }
        eprintln!("SD14 all {} sensor channels exact; offset={}", channels.pixels.len(), channels.legacy_offset);
        drop(channels);
        assert_eq!(budget.used(), 0);
        let entry = inventory.entries().find(|entry| &entry.kind == b"CAMF").unwrap();
        let camf = entry.camf().unwrap().unwrap();
        let calibration = camf.decode_type2(&budget, || false).unwrap();
        eprintln!("SD14 CAMF bytes={} AutoRGBNeutral enabled={:?}", calibration.len(), x3f_block_enabled(&calibration, b"AutoRGBNeutral", &mut || false));
        if let Some(folder) = std::env::var_os("RRRAH_X3F_SD14_NEUTRAL_ORACLES") {
            for mode in ["Auto", "Custom", "Sunlight", "Shade", "Overcast", "Incandescent", "Fluorescent", "Flash"] {
                let oracle = std::fs::read(std::path::PathBuf::from(&folder).join(format!("{mode}.bin"))).unwrap();
                assert_eq!(oracle.len(), 12);
                let response = x3f_neutral_response(&calibration, mode.as_bytes(), || false).unwrap();
                for (actual, expected) in response.iter().zip(oracle.chunks_exact(4)) {
                    assert_eq!(actual.to_bits(), f32::from_le_bytes(expected.try_into().unwrap()).to_bits(), "SD14 neutral {mode}");
                }
                eprintln!("SD14 calibrated neutral {mode} exact: {response:?}");
            }
            assert!(x3f_neutral_response(&calibration, b"Unknown", || false).is_err());
            assert!(matches!(x3f_neutral_response(&calibration, b"Auto", || true), Err(X3fError::Cancelled)));
        }
        if let Some(path) = std::env::var_os("RRRAH_X3F_SD14_LINEAR_ORACLE") {
            let processing_budget = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
            let channels = sensor.legacy_huffman(|| false).unwrap().decode_channels(&processing_budget, || false).unwrap();
            let target = [[1.4032, -0.2231, -0.1016], [-0.5263, 1.4816, 0.017], [-0.0112, 0.0183, 0.9113]];
            let area = [24, 16, 2663, 1773];
            let mode = std::env::var("RRRAH_X3F_SD14_WB").unwrap_or_else(|_| "Auto".into());
            let processed = channels.process_with_white_balance(&calibration, mode.as_bytes(), target, area, &processing_budget, || false).unwrap();
            drop(channels);
            let linear = x3f_linear_output(&processed, target, &processing_budget, || false).unwrap();
            drop(processed);
            let expected = std::fs::read(path).unwrap();
            let header = b"P6\n2639 1757\n65535\n";
            assert!(expected.starts_with(header));
            assert_eq!(expected.len()-header.len(), linear.len()*2);
            let mut differences = 0usize;
            let mut max_difference = 0u16;
            for (actual, expected) in linear.iter().zip(expected[header.len()..].chunks_exact(2)) {
                let expected = u16::from_be_bytes(expected.try_into().unwrap());
                differences += usize::from(*actual != expected);
                max_difference = max_difference.max(actual.abs_diff(expected));
            }
            eprintln!("SD14 {mode} full linear channels mismatches={differences} max difference={max_difference}");
            assert_eq!(differences, 0);
            if mode == "Sunlight" {
                let mut request = crate::DecodeRequest::new(std::path::PathBuf::from(std::env::var_os("RRRAH_X3F_SD14_SOURCE").unwrap()));
                request.memory_budget = Some(processing_budget.clone());
                let raster = crate::decode_raster(&request).unwrap();
                assert_eq!((raster.width(), raster.height()), (2639,1757));
                let rrrah_core::RasterPixels::Rgba16(values) = raster.pixels() else { panic!("precision") };
                for (actual, expected) in values.chunks_exact(4).zip(linear.chunks_exact(3)) {
                    assert_eq!(&actual[..3], expected); assert_eq!(actual[3],65535);
                }
                drop(raster);
                eprintln!("SD14 public raster selected Sunlight matches complete independent output");
            }
            drop(linear);
            assert_eq!(processing_budget.used(), 0);
        }
        drop(calibration);
        assert_eq!(budget.used(), 0);
        let property_entry = inventory.entries().find(|entry| &entry.kind == b"PROP").unwrap();
        let properties = property_entry.properties(|| false).unwrap().unwrap();
        let (key, value) = properties.entries().find(|(key, _)| *key == b"W\0B\0_\0D\0E\0S\0C\0").unwrap();
        let key_offset = key.as_ptr() as usize - source.as_ptr() as usize;
        let value_offset = value.as_ptr() as usize - source.as_ptr() as usize;
        for (offset, changed) in [(key_offset, b'X'), (value_offset+1, 0x80)] {
            let mut invalid = source.clone(); invalid[offset] = changed;
            let tiny = rrrah_core::MemoryBudget::new(1);
            assert!(matches!(decode_x3f_legacy(&invalid, &tiny, || false), Err(X3fError::Invalid)));
            assert_eq!(tiny.peak(), 0);
        }
        let mut duplicate = source.clone();
        let count = word(property_entry.bytes, 8) as usize;
        let strings_offset = property_entry.offset as usize + 24 + count*8;
        let wb_index = ((key_offset-strings_offset)/2) as u32;
        let table_offset = property_entry.offset as usize+24;
        duplicate[table_offset..table_offset+4].copy_from_slice(&wb_index.to_le_bytes());
        let tiny = rrrah_core::MemoryBudget::new(1);
        assert!(matches!(decode_x3f_legacy(&duplicate, &tiny, || false), Err(X3fError::Invalid)));
        assert_eq!(tiny.peak(), 0);
        let mut unknown = source.clone(); unknown[value_offset] = b'X';
        assert!(matches!(decode_x3f_legacy(&unknown, &budget, || false), Err(X3fError::Invalid)));
        assert_eq!(budget.used(), 0);
        assert!(matches!(decode_x3f_legacy(&source, &budget, || true), Err(X3fError::Cancelled)));
        eprintln!("SD14 missing duplicate non-ASCII unknown WB and cancellation refuse without fallback or retained memory");
        let raster = decode_x3f_legacy_auto(&source, &budget, || false).unwrap();
        assert_eq!((raster.width(), raster.height()), (2639,1757));
        drop(raster);
        assert_eq!(budget.used(), 0);
        eprintln!("SD14 explicit Auto container opening succeeds; all leases released");
    }

    #[test]
    fn rotation_clockwise_non_square_budget_cancel() {
        let rgb: Vec<u16> = (1..=6).flat_map(|v| [v; 3]).collect();
        let budget = rrrah_core::MemoryBudget::new(36);
        for (angle, expected) in [
            (0, vec![1, 2, 3, 4, 5, 6]),
            (90, vec![5, 3, 1, 6, 4, 2]),
            (180, vec![6, 5, 4, 3, 2, 1]),
            (270, vec![2, 4, 6, 1, 3, 5]),
        ] {
            let (out, w, h) = rotate_x3f_linear(&rgb, 2, 3, angle, &budget, || false).unwrap();
            assert_eq!((w, h), if angle % 180 == 0 { (2, 3) } else { (3, 2) });
            assert_eq!(out.chunks_exact(3).map(|p| p[0]).collect::<Vec<_>>(), expected);
            drop(out);
            assert_eq!(budget.used(), 0);
        }
        assert!(rotate_x3f_linear(&rgb, 2, 3, 45, &budget, || false).is_err());
        assert!(rotate_x3f_linear(&rgb, 2, 3, 90, &rrrah_core::MemoryBudget::new(35), || false).is_err());
        assert!(matches!(
            rotate_x3f_linear(&rgb, 2, 3, 90, &budget, || true),
            Err(X3fError::Cancelled)
        ));
    }
    #[test]
    fn linear_raster_preserves_values_color_alpha_and_lease() {
        let budget = rrrah_core::MemoryBudget::new(16);
        let raster = x3f_linear_raster(&[0, 123, 65535, 400, 500, 600], 2, 1, &budget, || false).unwrap();
        assert_eq!(raster.color_space(), &rrrah_core::RasterColorSpace::LinearSrgb);
        let rrrah_core::RasterPixels::Rgba16(values) = raster.pixels() else {
            panic!("precision lost")
        };
        assert_eq!(&values[..], &[0, 123, 65535, 65535, 400, 500, 600, 65535]);
        drop(raster);
        assert_eq!(budget.used(), 0);
        assert!(x3f_linear_raster(&[0; 3], 2, 1, &budget, || false).is_err());
        assert!(x3f_linear_raster(&[0; 6], 2, 1, &rrrah_core::MemoryBudget::new(15), || false).is_err());
    }
    #[test]
    fn linear_output_matrix_truncation_clamp_budget_release() {
        let budget = rrrah_core::MemoryBudget::new(6);
        let output = x3f_linear_output(
            &[3, 40000, 10],
            [[0.5, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, -1.0]],
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(&output[..], &[1, 65535, 0]);
        drop(output);
        assert_eq!(budget.used(), 0);
        assert!(x3f_linear_output(&[-1, 0, 0], [[1.0; 3]; 3], &budget, || false).is_err());
        assert_eq!(budget.used(), 0);
        assert!(matches!(
            x3f_linear_output(&[0; 3], [[1.0; 3]; 3], &budget, || true),
            Err(X3fError::Cancelled)
        ));
    }
    #[test]
    fn crop_channels_exact_rows_bounds_budget_cancel_and_lease() {
        let pixels: Vec<i32> = (0..4 * 3 * 3).collect();
        let budget = rrrah_core::MemoryBudget::new(48);
        let output = crop_x3f_channels(&pixels, 4, 3, [1, 1, 3, 3], &budget, || false).unwrap();
        assert_eq!(&output[..], &[15, 16, 17, 18, 19, 20, 27, 28, 29, 30, 31, 32]);
        let retained = output.clone();
        drop(output);
        assert_eq!(budget.used(), 48);
        drop(retained);
        assert_eq!(budget.used(), 0);
        for area in [[1, 1, 1, 2], [0, 2, 3, 1], [0, 0, 5, 3], [0, 0, 4, 4]] {
            assert!(crop_x3f_channels(&pixels, 4, 3, area, &budget, || false).is_err());
        }
        assert!(
            crop_x3f_channels(
                &pixels,
                4,
                3,
                [1, 1, 3, 3],
                &rrrah_core::MemoryBudget::new(47),
                || false
            )
            .is_err()
        );
        let mut calls = 0;
        assert!(matches!(
            crop_x3f_channels(&pixels, 4, 3, [1, 1, 3, 3], &budget, || {
                calls += 1;
                calls == 3
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn chroma_smoothing_zero_border_budget_and_cancellation() {
        let budget = rrrah_core::MemoryBudget::new(65536);
        let curve = X3fNoiseCurve::build(10.0, 5.0, 0.8, &budget, || false).unwrap();
        let held = budget.used();
        let mut pixels = vec![0; 5 * 9 * 3];
        for y in 0..9 {
            for x in 0..5 {
                if x == 4 || y == 8 {
                    pixels[(y * 5 + x) * 3..(y * 5 + x) * 3 + 3].fill(100);
                }
            }
        }
        let before = pixels.clone();
        smooth_x3f_chroma(&mut pixels, 5, 9, [&curve; 3], &budget, || false).unwrap();
        assert_eq!(pixels, before);
        assert_eq!(budget.used(), held);
        let mut calls = 0;
        assert!(matches!(
            smooth_x3f_chroma(&mut pixels, 5, 9, [&curve; 3], &budget, || {
                calls += 1;
                calls == 8
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(budget.used(), held);
        assert!(
            smooth_x3f_chroma(
                &mut pixels,
                5,
                9,
                [&curve; 3],
                &rrrah_core::MemoryBudget::new(12),
                || false
            )
            .is_err()
        );
        drop(curve);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn chroma_guide_bottom_up_partial_blocks_budget_and_cancel() {
        let budget = rrrah_core::MemoryBudget::new(12);
        let mut pixels = vec![0; 5 * 9 * 3];
        for y in 0..9 {
            for x in 0..5 {
                for c in 0..3 {
                    pixels[(y * 5 + x) * 3 + c] = if x == 4 || y == 8 {
                        24000
                    } else if y < 4 {
                        1000
                    } else {
                        2000
                    };
                }
            }
        }
        let guide = x3f_chroma_guide(&pixels, 5, 9, &budget, || false).unwrap();
        assert_eq!(&guide[..], &[1449, 1449, 1449, 2000, 2000, 2000]);
        assert_eq!(budget.used(), 12);
        let retained = guide.clone();
        drop(guide);
        assert_eq!(budget.used(), 12);
        drop(retained);
        assert_eq!(budget.used(), 0);
        assert!(x3f_chroma_guide(&pixels, 5, 9, &rrrah_core::MemoryBudget::new(11), || false).is_err());
        let mut calls = 0;
        assert!(matches!(
            x3f_chroma_guide(&pixels, 5, 9, &budget, || {
                calls += 1;
                calls == 4
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        pixels[0] = -1;
        assert!(x3f_chroma_guide(&pixels, 5, 9, &budget, || false).is_err());
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn pixel_transform_rounding_clamps_validation_and_cancel() {
        let budget = rrrah_core::MemoryBudget::new(65536);
        let curve = X3fNoiseCurve::build(10.0, 5.0, 0.8, &budget, || false).unwrap();
        let transform = X3fColorTransform {
            coefficients: [[1.0009765625, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 1.0]],
            luminance: 1.0,
        };
        let mut pixels = [1000, 20000, -1000];
        transform_x3f_pixels(&mut pixels, [&curve; 3], &transform, || false).unwrap();
        assert_eq!(pixels, [1001, 24000, 0]);
        let mut bad = [100, 200, i32::MAX];
        let before = bad;
        assert!(transform_x3f_pixels(&mut bad, [&curve; 3], &transform, || false).is_err());
        assert_eq!(bad, before);
        assert!(matches!(
            transform_x3f_pixels(&mut pixels, [&curve; 3], &transform, || true),
            Err(X3fError::Cancelled)
        ));
        let mut invalid = transform;
        invalid.coefficients[0][0] = f32::NAN;
        assert!(transform_x3f_pixels(&mut pixels, [&curve; 3], &invalid, || false).is_err());
        drop(curve);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn wide_hue_smoothing_constant_impulse_border_and_release() {
        let budget = rrrah_core::MemoryBudget::new(65536);
        let curve = X3fNoiseCurve::build(50.0, 50.0, 0.8, &budget, || false).unwrap();
        let held = budget.used();
        let mut pixels = vec![100i32; 7 * 7 * 3];
        let before = pixels.clone();
        smooth_x3f_hues_wide(&mut pixels, 7, 7, &curve, &budget, || false).unwrap();
        assert_eq!(pixels, before);
        assert_eq!(budget.used(), held);
        pixels[(3 * 7 + 3) * 3] = 120;
        let before = pixels.clone();
        smooth_x3f_hues_wide(&mut pixels, 7, 7, &curve, &budget, || false).unwrap();
        assert!(pixels[(3 * 7 + 3) * 3] < 120);
        for y in 0..7 {
            for x in 0..7 {
                if x < 2 || y < 2 || x >= 5 || y >= 5 {
                    let i = (y * 7 + x) * 3;
                    assert_eq!(&pixels[i..i + 3], &before[i..i + 3]);
                }
            }
        }
        assert!(matches!(
            smooth_x3f_hues_wide(
                &mut pixels,
                7,
                7,
                &curve,
                &rrrah_core::MemoryBudget::new(0),
                || false
            ),
            Err(X3fError::Memory(_))
        ));
        assert_eq!(budget.used(), held);
        drop(curve);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn hue_smoothing_constant_impulse_edges_budget_and_cancel() {
        let budget = rrrah_core::MemoryBudget::new(65536);
        let curve = X3fNoiseCurve::build(50.0, 50.0, 0.8, &budget, || false).unwrap();
        let held = budget.used();
        let mut pixels = vec![100i32; 7 * 7 * 3];
        let before = pixels.clone();
        smooth_x3f_hues(&mut pixels, 7, 7, &curve, &budget, || false).unwrap();
        assert_eq!(pixels, before);
        assert_eq!(budget.used(), held);
        pixels[(3 * 7 + 3) * 3] = 120;
        let before = pixels.clone();
        smooth_x3f_hues(&mut pixels, 7, 7, &curve, &budget, || false).unwrap();
        assert!(pixels[(3 * 7 + 3) * 3] < 120);
        for y in 0..7 {
            for x in 0..7 {
                if x < 2 || y < 2 || x >= 5 || y >= 5 {
                    let i = (y * 7 + x) * 3;
                    assert_eq!(&pixels[i..i + 3], &before[i..i + 3]);
                }
            }
        }
        assert!(matches!(
            smooth_x3f_hues(
                &mut pixels,
                7,
                7,
                &curve,
                &rrrah_core::MemoryBudget::new(0),
                || false
            ),
            Err(X3fError::Memory(_))
        ));
        let mut calls = 0;
        assert!(matches!(
            smooth_x3f_hues(&mut pixels, 7, 7, &curve, &budget, || {
                calls += 1;
                calls == 4
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(budget.used(), held);
        drop(curve);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn noise_curve_domain_symmetry_finite_budget_and_cancellation() {
        let budget = rrrah_core::MemoryBudget::new(65536);
        let curve = X3fNoiseCurve::build(10.0, 5.0, 0.8, &budget, || false).unwrap();
        assert_eq!(curve.domain(), 157);
        assert_eq!(curve.apply(0), 0);
        for i in 1..157 {
            assert_eq!(curve.apply(-i), -curve.apply(i));
        }
        assert_eq!(curve.apply(157), 0);
        assert_eq!(curve.apply(i32::MIN), 0);
        let default = X3fNoiseCurve::build(10.0, 5.0, 0.0, &budget, || false).unwrap();
        for i in -157..158 {
            assert_eq!(curve.apply(i), default.apply(i));
        }
        drop(default);
        drop(curve);
        assert_eq!(budget.used(), 0);
        for (a, b, f) in [
            (0.0, 5.0, 0.8),
            (10.0, 0.0, 0.8),
            (f64::NAN, 5.0, 0.8),
            (10.0, 5.0, -1.0),
        ] {
            assert!(X3fNoiseCurve::build(a, b, f, &budget, || false).is_err());
        }
        assert!(matches!(
            X3fNoiseCurve::build(10.0, 5.0, 0.8, &rrrah_core::MemoryBudget::new(0), || false),
            Err(X3fError::Memory(_))
        ));
        let mut calls = 0;
        assert!(matches!(
            X3fNoiseCurve::build(10.0, 5.0, 0.8, &budget, || {
                calls += 1;
                calls == 2
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn highlights_threshold_transition_saturation_and_invalid_calibration() {
        let mut values = [900, 1000, 1100, 1350, 1500, 1600, 1800, 1900, 2000];
        linearize_x3f_highlights(&mut values, [1600; 3], [1.0; 3], || false).unwrap();
        assert_eq!(values, [900, 1000, 1100, 1490, 1556, 1600, 2000, 2000, 2000]);
        assert!(linearize_x3f_highlights(&mut values, [0; 3], [1.0; 3], || false).is_err());
        assert!(linearize_x3f_highlights(&mut values, [1600; 3], [f32::NAN; 3], || false).is_err());
        assert!(linearize_x3f_highlights(&mut values[..8], [1600; 3], [1.0; 3], || false).is_err());
        let before = values;
        assert!(matches!(
            linearize_x3f_highlights(&mut values, [1600; 3], [1.0; 3], || true),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(before, values);
    }

    #[test]
    fn red_sharpen_constant_impulse_edges_other_channels_budget_and_cancel() {
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut pixels = vec![100i32; 7 * 7 * 3];
        let original = pixels.clone();
        sharpen_x3f_red(&mut pixels, 7, 7, &budget, || false).unwrap();
        assert_eq!(pixels, original);
        assert_eq!(budget.used(), 0);
        pixels[(3 * 7 + 3) * 3] = 1000;
        let before = pixels.clone();
        sharpen_x3f_red(&mut pixels, 7, 7, &budget, || false).unwrap();
        assert!(pixels[(3 * 7 + 3) * 3] > 1000);
        for y in 0..7 {
            for x in 0..7 {
                let i = (y * 7 + x) * 3;
                assert_eq!(&pixels[i + 1..i + 3], &before[i + 1..i + 3]);
                if x < 2 || y < 2 || x >= 5 || y >= 5 {
                    assert_eq!(pixels[i], before[i]);
                }
            }
        }
        assert_eq!(budget.used(), 0);
        assert!(matches!(
            sharpen_x3f_red(&mut pixels, 7, 7, &rrrah_core::MemoryBudget::new(0), || false),
            Err(X3fError::Memory(_))
        ));
        let before = pixels.clone();
        let mut calls = 0;
        assert!(matches!(
            sharpen_x3f_red(&mut pixels, 7, 7, &budget, || {
                calls += 1;
                calls == 4
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(pixels, before);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn bad_pixels_neighbor_mask_origin_edges_signed_means_and_cancel() {
        let mut pixels = vec![0i32; 27];
        pixels[..3].copy_from_slice(&[10, 20, -11]);
        pixels[24..].copy_from_slice(&[30, 40, -30]);
        pixels[12..15].copy_from_slice(&[999; 3]);
        let code = ((1u32 << 20) | (1 << 8) | 0x81).to_le_bytes();
        assert_eq!(
            repair_x3f_bad_pixels(&mut pixels, 3, 3, [0, 0], &code, || false).unwrap(),
            1
        );
        assert_eq!(&pixels[12..15], &[20, 30, -20]);
        let translated = ((3u32 << 20) | (4 << 8) | 0x81).to_le_bytes();
        assert_eq!(
            repair_x3f_bad_pixels(&mut pixels, 3, 3, [3, 2], &translated, || false).unwrap(),
            1
        );
        assert_eq!(
            repair_x3f_bad_pixels(&mut pixels, 3, 3, [0, 0], &[0; 4], || false).unwrap(),
            0
        );
        assert!(repair_x3f_bad_pixels(&mut pixels, 3, 3, [0, 0], &[0; 3], || false).is_err());
        assert!(matches!(
            repair_x3f_bad_pixels(&mut pixels, 3, 3, [0, 0], &code, || true),
            Err(X3fError::Cancelled)
        ));
    }

    #[test]
    fn black_scene_adjustment_sample_stride_signed_values_and_refusal() {
        let budget = rrrah_core::MemoryBudget::new(1024);
        let mut pixels = budget.try_buffer(27, 0u16).unwrap();
        pixels[24] = 100;
        pixels[25] = 200;
        pixels[26] = (-300i16) as u16;
        let frame = X3fLegacyChannels {
            width: 3,
            height: 3,
            legacy_offset: 0,
            pixels: pixels.freeze().into(),
        };
        let mut black = [0f32; 9];
        assert_eq!(
            frame
                .adjust_black_scene(&mut black, [2.0, 4.0, 6.0], || false)
                .unwrap(),
            [2.0, 4.0, 0.0]
        );
        assert_eq!(black, [2.0, 4.0, 0.0, 2.0, 4.0, 0.0, 2.0, 4.0, 0.0]);
        assert!(
            frame
                .adjust_black_scene(&mut black[..8], [0.0; 3], || false)
                .is_err()
        );
        assert!(
            frame
                .adjust_black_scene(&mut black, [f32::NAN; 3], || false)
                .is_err()
        );
        let before = black;
        assert!(matches!(
            frame.adjust_black_scene(&mut black, [0.0; 3], || true),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(black, before);
        drop(frame);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn black_smoothing_constant_outlier_invalid_and_cancel() {
        let mut constant = vec![4f32; 90];
        let mean = smooth_x3f_black_rows(&mut constant, || false).unwrap();
        assert_eq!(mean, [4.0; 3]);
        assert!(constant.iter().all(|v| v.is_finite()));
        let mut spike = vec![4f32; 90];
        spike[15 * 3] = 1000.0;
        let mean = smooth_x3f_black_rows(&mut spike, || false).unwrap();
        assert_eq!(mean, [4.0; 3]);
        assert_eq!(spike, constant);
        assert!(smooth_x3f_black_rows(&mut [0f32; 63], || false).is_err());
        let mut bad = vec![0f32; 90];
        bad[20] = f32::NAN;
        let before = bad.clone();
        assert!(smooth_x3f_black_rows(&mut bad, || false).is_err());
        assert!(bad.iter().zip(before).all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(matches!(
            smooth_x3f_black_rows(&mut constant, || true),
            Err(X3fError::Cancelled)
        ));
    }

    #[test]
    fn shield_columns_filter_trim_and_invalid_ranges() {
        let budget = rrrah_core::MemoryBudget::new(1024);
        let mut pixels = budget.try_buffer(15, 0u16).unwrap();
        pixels.copy_from_slice(&[0, 0, 0, 1, 2, 65535, 3, 4, 65533, 100, 101, 65436, 5, 6, 65531]);
        let frame = X3fLegacyChannels {
            width: 5,
            height: 1,
            legacy_offset: 0,
            pixels: pixels.freeze().into(),
        };
        assert_eq!(
            frame.shield_column_mean(0, [1, 4], 0.0, || false).unwrap(),
            [4.0, 5.0, -4.0]
        );
        assert_eq!(
            frame.shield_column_mean(0, [1, 2], 1.0, || false).unwrap(),
            [3.5, 5.0, -3.5]
        );
        for (row, range) in [(1, [1, 2]), (0, [0, 2]), (0, [2, 2]), (0, [3, 2]), (0, [1, 5])] {
            assert!(frame.shield_column_mean(row, range, 0.0, || false).is_err());
        }
        assert!(frame.shield_column_mean(0, [1, 2], f32::NAN, || false).is_err());
        assert!(matches!(
            frame.shield_column_mean(0, [1, 2], 0.0, || true),
            Err(X3fError::Cancelled)
        ));
        drop(frame);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn shield_mean_signed_samples_bounds_and_cancellation() {
        let budget = rrrah_core::MemoryBudget::new(1024);
        let mut pixels = budget.try_buffer(12, 0u16).unwrap();
        pixels.copy_from_slice(&[1, 2, 65535, 3, 4, 65533, 5, 6, 65531, 7, 8, 65529]);
        let frame = X3fLegacyChannels {
            width: 2,
            height: 2,
            legacy_offset: 0,
            pixels: pixels.freeze().into(),
        };
        assert_eq!(
            frame.shield_mean([0, 0, 1, 1], || false).unwrap(),
            [4.0, 5.0, -4.0]
        );
        assert_eq!(
            frame.shield_mean([1, 1, 1, 1], || false).unwrap(),
            [7.0, 8.0, -7.0]
        );
        for r in [[0, 0, 2, 1], [0, 0, 1, 2], [1, 0, 0, 1], [0, 1, 1, 0]] {
            assert!(frame.shield_mean(r, || false).is_err());
        }
        assert!(matches!(
            frame.shield_mean([0, 0, 1, 1], || true),
            Err(X3fError::Cancelled)
        ));
        let used = budget.used();
        let mut calls = 0;
        assert!(matches!(
            frame.shield_mean([0, 0, 1, 1], || {
                calls += 1;
                calls == 3
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(budget.used(), used);
        drop(frame);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn dark_drift_interpolates_all_channels_and_rejects_bad_rows() {
        let mut dims = Vec::new();
        for v in [2u32, 0, 0, 3, 0, 1, 2, 0, 2] {
            dims.extend_from_slice(&v.to_le_bytes());
        }
        let data: Vec<_> = [1f32, 2.0, 3.0, 4.0, 5.0, 6.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let m = X3fCamfMatrix {
            element_type: 3,
            dimensions: &dims,
            data: &data,
            elements: 12,
        };
        assert_eq!(m.dark_drift(0, 3).unwrap(), [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]);
        assert_eq!(
            m.dark_drift(1, 3).unwrap(),
            [[6.0, 7.0], [8.0, 9.0], [10.0, 11.0]]
        );
        assert_eq!(
            m.dark_drift(2, 3).unwrap(),
            [[11.0, 12.0], [13.0, 14.0], [15.0, 16.0]]
        );
        for (row, height) in [(0, 0), (0, 1), (3, 3), (u32::MAX, 3)] {
            assert!(m.dark_drift(row, height).is_err());
        }
        let mut bad = data.clone();
        bad[..4].copy_from_slice(&f32::INFINITY.to_le_bytes());
        assert!(X3fCamfMatrix { data: &bad, ..m }.dark_drift(1, 3).is_err());
    }

    #[test]
    fn post_polynomial_fixed_point_terms_and_overflow() {
        let mut dims = Vec::new();
        for v in [3u32, 0, 0, 9, 0, 1] {
            dims.extend_from_slice(&v.to_le_bytes());
        }
        let mut data = vec![0u8; 108];
        let matrix = X3fCamfMatrix {
            element_type: 3,
            dimensions: &dims,
            data: &data,
            elements: 27,
        };
        assert_eq!(
            matrix.post_polynomial([16384, 8192, -4096]).unwrap(),
            [16384, 8192, -4096]
        );
        data[0..4].copy_from_slice(&0.5f32.to_le_bytes());
        data[12..16].copy_from_slice(&1.0f32.to_le_bytes());
        let matrix = X3fCamfMatrix {
            element_type: 3,
            dimensions: &dims,
            data: &data,
            elements: 27,
        };
        // Square of red / 2 plus red*green cross term (legacy reversed order).
        assert_eq!(
            matrix.post_polynomial([16384, 8192, -4096]).unwrap(),
            [32768, 8192, -4096]
        );
        assert!(matches!(
            matrix.post_polynomial([i32::MAX; 3]),
            Err(X3fError::Limit)
        ));
        let mut bad = data.clone();
        bad[..4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(
            X3fCamfMatrix { data: &bad, ..matrix }
                .post_polynomial([1; 3])
                .is_err()
        );
    }

    #[test]
    fn spatial_gain_bilinear_grid_boundaries_and_invalid_values() {
        let mut dims = Vec::new();
        for value in [2u32, 0, 1, 2, 0, 2, 3, 0, 0] {
            dims.extend_from_slice(&value.to_le_bytes());
        }
        let data: Vec<_> = [1f32, 2.0, 3.0, 3.0, 4.0, 5.0, 5.0, 6.0, 7.0, 7.0, 8.0, 9.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let matrix = X3fCamfMatrix {
            element_type: 3,
            dimensions: &dims,
            data: &data,
            elements: 12,
        };
        assert_eq!(matrix.spatial_gain(0.0, 0.0).unwrap(), [1.0, 2.0, 3.0]);
        assert_eq!(matrix.spatial_gain(1.0, 1.0).unwrap(), [7.0, 8.0, 9.0]);
        assert_eq!(matrix.spatial_gain(0.5, 0.5).unwrap(), [4.0, 5.0, 6.0]);
        for (x, y) in [(f32::NAN, 0.0), (0.0, f32::INFINITY), (-0.1, 0.0), (1.1, 0.0)] {
            assert!(matrix.spatial_gain(x, y).is_err());
        }
        let mut bad = data.clone();
        bad[..4].copy_from_slice(&0f32.to_le_bytes());
        assert!(
            X3fCamfMatrix { data: &bad, ..matrix }
                .spatial_gain(0.0, 0.0)
                .is_err()
        );
    }

    #[test]
    fn camf_property_links_refuse_truncation_offsets_and_cancel() {
        let mut raw = vec![0u8; 44];
        raw[20..24].copy_from_slice(&1u32.to_le_bytes());
        raw[24..28].copy_from_slice(&36u32.to_le_bytes());
        raw[32..36].copy_from_slice(&2u32.to_le_bytes());
        raw[36..40].copy_from_slice(b"A\0B\0");
        fn visit(raw: &[u8]) -> Result<bool, X3fError> {
            X3fCamfEntry {
                raw,
                kind: *b"CMbP",
                version: 0x10000,
                name: b"P",
                value: raw.get(20..).unwrap_or(&[]),
            }
            .visit_properties(
                || false,
                |k, v| {
                    assert_eq!(k, b"A");
                    assert_eq!(v, b"B");
                },
            )
        }
        assert_eq!(visit(&raw).unwrap(), true);
        for end in 0..40 {
            assert!(visit(&raw[..end]).is_err());
        }
        for (offset, value) in [
            (20, 1025u32),
            (24, 35),
            (24, u32::MAX),
            (28, u32::MAX),
            (32, u32::MAX),
        ] {
            let mut bad = raw.clone();
            bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(visit(&bad).is_err());
        }
        let entry = X3fCamfEntry {
            raw: &raw,
            kind: *b"CMbP",
            version: 0x10000,
            name: b"P",
            value: &raw[20..],
        };
        assert!(matches!(
            entry.visit_properties(|| true, |_, _| panic!()),
            Err(X3fError::Cancelled)
        ));
    }

    #[test]
    fn camf_float_access_refuses_nonfinite_wrong_type_and_bad_indices() {
        let values = [1.25f32, f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
        let bytes: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let matrix = X3fCamfMatrix {
            element_type: 3,
            dimensions: &[],
            data: &bytes,
            elements: 4,
        };
        assert_eq!(matrix.float32(0).unwrap(), 1.25);
        for i in [1, 2, 3, 4, usize::MAX] {
            assert!(matrix.float32(i).is_err());
        }
        let integer = X3fCamfMatrix {
            element_type: 2,
            ..matrix
        };
        assert!(integer.float32(0).is_err());
        let truncated = X3fCamfMatrix {
            element_type: 3,
            data: &bytes[..3],
            ..integer
        };
        assert!(truncated.float32(0).is_err());
    }

    #[test]
    fn camf_matrix_rejects_corrupt_dimensions_ranges_and_overflow() {
        // Two axes stored in reversed order, with four explicit f32 values.
        let mut raw = vec![0u8; 80];
        raw[..4].copy_from_slice(b"CMbM");
        raw[20..22].copy_from_slice(b"M\0");
        for (offset, value) in [
            (24, 3u32),
            (28, 2),
            (32, 64),
            (36, 2),
            (40, 20),
            (44, 1),
            (48, 2),
            (52, 20),
            (56, 0),
        ] {
            raw[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (index, value) in [1.0f32, 2.0, 3.0, 4.0].into_iter().enumerate() {
            raw[64 + index * 4..68 + index * 4].copy_from_slice(&value.to_le_bytes());
        }
        fn parse(raw: &[u8]) -> Result<Option<X3fCamfMatrix<'_>>, X3fError> {
            X3fCamfEntry {
                raw,
                kind: *b"CMbM",
                version: 0x10000,
                name: b"M",
                value: raw.get(24..).unwrap_or(&[]),
            }
            .matrix()
        }
        let matrix = parse(&raw).unwrap().unwrap();
        assert_eq!((matrix.elements, matrix.data.len()), (4, 16));
        for end in 0..raw.len() {
            assert!(parse(&raw[..end]).is_err(), "prefix {end}");
        }
        for (offset, value) in [
            (24, 4u32),
            (28, 0),
            (28, 9),
            (32, 59),
            (32, u32::MAX),
            (36, 0),
            (40, 0),
            (40, u32::MAX),
            (44, 2),
            (56, 1),
        ] {
            let mut bad = raw.clone();
            bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(parse(&bad).is_err(), "offset {offset} value {value}");
        }
        let mut huge = raw.clone();
        huge[36..40].copy_from_slice(&u32::MAX.to_le_bytes());
        huge[48..52].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(parse(&huge), Err(X3fError::Limit)));
        for (kind, width) in [(0u32, 2usize), (1, 4), (2, 4), (3, 4), (5, 1), (6, 2)] {
            let mut typed = raw.clone();
            typed[24..28].copy_from_slice(&kind.to_le_bytes());
            assert_eq!(parse(&typed).unwrap().unwrap().data.len(), 4 * width);
        }
    }

    #[test]
    fn camf_type2_budget_cancellation_and_round_trip() {
        let bytes = [1u8; 4096];
        let camf = X3fCamf {
            encoding: 2,
            parameters: [0, 0, 0, 1632179179],
            payload: &bytes,
        };
        let budget = rrrah_core::MemoryBudget::new(8192);
        let decoded = camf.decode_type2(&budget, || false).unwrap();
        let reverse = X3fCamf {
            payload: &decoded,
            ..camf
        }
        .decode_type2(&budget, || false)
        .unwrap();
        assert_eq!(&*reverse, &bytes);
        drop(reverse);
        drop(decoded);
        assert_eq!(budget.used(), 0);
        assert!(matches!(
            camf.decode_type2(&rrrah_core::MemoryBudget::new(0), || false),
            Err(X3fError::Memory(_))
        ));
        let mut calls = 0;
        assert!(matches!(
            camf.decode_type2(&budget, || {
                calls += 1;
                calls == 3
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        assert!(
            X3fCamf { encoding: 4, ..camf }
                .decode_type2(&budget, || false)
                .is_err()
        );
    }

    #[test]
    fn properties_validate_offsets_unicode_and_cancellation() {
        let mut b = Vec::from(&b"SECp"[..]);
        for w in [0x20000u32, 1, 0, 0, 4, 0, 2] {
            b.extend_from_slice(&w.to_le_bytes());
        }
        b.extend_from_slice(&[b'A', 0, 0, 0, b'B', 0, 0, 0]);
        let parse = |bytes| {
            X3fEntry {
                kind: *b"PROP",
                offset: 0,
                bytes,
            }
            .properties(|| false)
        };
        assert_eq!(
            parse(&b).unwrap().unwrap().entries().next(),
            Some((&b"A\0"[..], &b"B\0"[..]))
        );
        for end in 0..b.len() {
            assert!(parse(&b[..end]).is_err());
        }
        let mut bad = b.clone();
        bad[28..32].copy_from_slice(&4u32.to_le_bytes());
        assert!(parse(&bad).is_err());
        let mut bad = b.clone();
        bad[32..34].copy_from_slice(&0xd800u16.to_le_bytes());
        assert!(parse(&bad).is_err());
        assert!(matches!(
            X3fEntry {
                kind: *b"PROP",
                offset: 0,
                bytes: &b
            }
            .properties(|| true),
            Err(X3fError::Cancelled)
        ));
    }

    #[test]
    #[ignore = "requires pinned Sigma SD10 X3F and independent LibRaw channel dump"]
    fn sigma_sd10_inventory_distinguishes_sensor_from_previews() {
        let bytes = std::fs::read(std::env::var_os("RRRAH_X3F_SOURCE").expect("RRRAH_X3F_SOURCE")).unwrap();
        let inventory = inspect_x3f(&bytes, || false).unwrap();
        assert_eq!(
            (inventory.version, inventory.width, inventory.height),
            (0x0002_0002, 2268, 1512)
        );
        assert_eq!(inventory.entries().len(), 5);
        let properties = inventory
            .entries()
            .find(|e| &e.kind == b"PROP")
            .unwrap()
            .properties(|| false)
            .unwrap()
            .unwrap();
        let text = |bytes: &[u8]| {
            String::from_utf16(
                &bytes
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        };
        let pairs: Vec<_> = properties.entries().map(|(k, v)| (text(k), text(v))).collect();
        assert_eq!(pairs.len(), 26);
        for (name, value) in [
            ("CAMMANUF", "SIGMA"),
            ("CAMMODEL", "SIGMA SD10"),
            ("WB_DESC", "Auto"),
            ("ISO", "100"),
        ] {
            assert!(pairs.iter().any(|(k, v)| k == name && v == value));
        }
        let entry = inventory.entries().find(|e| &e.kind == b"CAMF").unwrap();
        let camf = entry.camf().unwrap().unwrap();
        assert_eq!(camf.encoding, 2);
        assert_eq!(camf.parameters, [0, 1648706374, 65536, 1632179179]);
        assert_eq!(camf.payload.len(), 52520);
        let calibration_budget = rrrah_core::MemoryBudget::new(65536);
        let calibration = camf.decode_type2(&calibration_budget, || false).unwrap();
        assert_eq!(&calibration[..4], b"CMbT");
        let oracle =
            std::fs::read(std::env::var_os("RRRAH_X3F_CAMF_ORACLE").expect("RRRAH_X3F_CAMF_ORACLE")).unwrap();
        assert_eq!(&*calibration, &oracle[..]);
        let embedded = unique_camf_matrix(&calibration, b"AutoRGBNeutral", || false).unwrap();
        assert_eq!(x3f_neutral_response(&calibration, b"Auto", || false).unwrap(),
                   [embedded.float32(0).unwrap(), embedded.float32(1).unwrap(), embedded.float32(2).unwrap()]);
        eprintln!("SD10 explicit neutral resolver preserves embedded Auto response exactly");
        let identity = [[1.0f32, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let transform = x3f_color_transform_auto(&calibration, identity, || false).unwrap();
        assert!(transform.luminance > 0.0);
        if let Some(path) = std::env::var_os("RRRAH_X3F_TRANSFORM_ORACLE") {
            let oracle = std::fs::read(path).unwrap();
            assert_eq!(oracle.len(), 44);
            assert_eq!(
                transform.luminance,
                f64::from_le_bytes(oracle[..8].try_into().unwrap())
            );
            for (actual, bytes) in transform
                .coefficients
                .iter()
                .flatten()
                .zip(oracle[8..].chunks_exact(4))
            {
                assert_eq!(*actual, f32::from_le_bytes(bytes.try_into().unwrap()));
            }
            eprintln!("SD10 transform coefficients and luminance exact for explicit identity target");
        }

        assert!(transform.coefficients.iter().flatten().all(|v| v.is_finite()));
        assert!(x3f_color_transform_auto(&calibration, [[0.0; 3]; 3], || false).is_err());
        assert!(x3f_color_transform_auto(&calibration, [[f32::NAN; 3]; 3], || false).is_err());
        assert!(matches!(
            x3f_color_transform_auto(&calibration, identity, || true),
            Err(X3fError::Cancelled)
        ));
        if let Some(path) = std::env::var_os("RRRAH_X3F_LEGACY_TRANSFORM_ORACLE") {
            // Explicit reference input from the legacy Foveon color convention.
            let legacy = x3f_color_transform_auto(
                &calibration,
                [
                    [1.4032, -0.2231, -0.1016],
                    [-0.5263, 1.4816, 0.017],
                    [-0.0112, 0.0183, 0.9113],
                ],
                || false,
            )
            .unwrap();
            let expected = std::fs::read(path).unwrap();
            assert_eq!(expected.len(), 44);
            assert_eq!(
                legacy.luminance,
                f64::from_le_bytes(expected[..8].try_into().unwrap())
            );
            for (i, value) in legacy.coefficients.iter().flatten().enumerate() {
                assert_eq!(
                    *value,
                    f32::from_le_bytes(expected[8 + i * 4..12 + i * 4].try_into().unwrap()),
                    "legacy target coefficient {i}"
                );
            }
            eprintln!("SD10 legacy Foveon target transform exact");
        }
        let derived_budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let derived_curves =
            x3f_noise_curves_auto(&calibration, transform.luminance, &derived_budget, || false).unwrap();
        if let Some(path) = std::env::var_os("RRRAH_X3F_DERIVED_CURVES_ORACLE") {
            let expected = std::fs::read(path).unwrap();
            let mut position = 0usize;
            for (index, curve) in derived_curves.iter().enumerate() {
                let size = word(&expected, position) as usize;
                position += 4;
                assert_eq!(size, curve.domain());
                for i in 0..size {
                    let value = i16::from_le_bytes(expected[position..position + 2].try_into().unwrap());
                    position += 2;
                    assert_eq!(
                        curve.apply(i as i32),
                        i32::from(value),
                        "derived curve {index} sample {i}"
                    );
                }
            }
            assert_eq!(position, expected.len());
            eprintln!("SD10 eight noise curves exact at independently derived luminance");
        }
        drop(derived_curves);
        assert_eq!(derived_budget.used(), 0);

        let curves_budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let curves = x3f_noise_curves_auto(&calibration, 1.0, &curves_budget, || false).unwrap();
        assert!(curves.iter().all(|c| c.domain() > 0));
        if let Some(path) = std::env::var_os("RRRAH_X3F_CURVES_ORACLE") {
            let oracle = std::fs::read(path).unwrap();
            let mut position = 0;
            for (index, curve) in curves.iter().enumerate() {
                let size = word(&oracle, position) as usize;
                position += 4;
                assert_eq!(size, curve.domain());
                for i in 0..size {
                    let expected = i16::from_le_bytes(oracle[position..position + 2].try_into().unwrap());
                    position += 2;
                    assert_eq!(
                        curve.apply(i as i32),
                        i32::from(expected),
                        "curve {index} sample {i}"
                    );
                }
            }
            assert_eq!(position, oracle.len());
            eprintln!("SD10 eight noise curves exact at explicit luminance=1");
        }

        assert_eq!(curves[6].apply(0), 0);
        assert!(curves_budget.used() > 0);
        drop(curves);
        assert_eq!(curves_budget.used(), 0);
        assert!(matches!(
            x3f_noise_curves_auto(&calibration, 1.0, &rrrah_core::MemoryBudget::new(0), || false),
            Err(X3fError::Memory(_))
        ));
        assert!(x3f_noise_curves_auto(&calibration, f64::NAN, &curves_budget, || false).is_err());
        let mut calls = 0;
        assert!(matches!(
            x3f_noise_curves_auto(&calibration, 1.0, &curves_budget, || {
                calls += 1;
                calls == 400
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(curves_budget.used(), 0);

        for block in [b"DarkDrift".as_slice(), b"AutoRGBNeutral", b"ColumnFilter"] {
            assert!(x3f_block_enabled(&calibration, block, || false).unwrap());
        }
        assert!(!x3f_block_enabled(&calibration, b"missing", || false).unwrap());
        assert!(matches!(
            x3f_block_enabled(&calibration, b"DarkDrift", || true),
            Err(X3fError::Cancelled)
        ));

        assert_eq!(
            x3f_wb_correction(&calibration, b"Auto", || false).unwrap(),
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
        );
        assert!(x3f_wb_correction(&calibration, b"missing", || false).is_err());
        let illuminant = x3f_illuminant_matrix(&calibration, b"Auto", || false).unwrap();
        assert!(illuminant.iter().all(|v| v.is_finite()));
        assert_eq!(
            illuminant,
            x3f_illuminant_matrix(&calibration, b"Flash", || false).unwrap()
        );
        assert!(x3f_illuminant_matrix(&calibration, b"missing", || false).is_err());

        assert!(matches!(
            x3f_wb_correction(&calibration, b"Auto", || true),
            Err(X3fError::Cancelled)
        ));
        let mut duplicated = calibration.to_vec();
        duplicated.extend_from_slice(&calibration);
        assert!(x3f_block_enabled(&duplicated, b"DarkDrift", || false).is_err());

        assert!(x3f_wb_correction(&duplicated, b"Auto", || false).is_err());

        let mut entries = 0;
        let mut matrices = 0;
        visit_x3f_camf(
            &calibration,
            || false,
            |entry| {
                entries += 1;
                if entry.name == b"WhiteBalanceCorrections" {
                    let mut links = 0;
                    let mut auto = false;
                    assert!(
                        entry
                            .visit_properties(
                                || false,
                                |key, value| {
                                    links += 1;
                                    if key == b"Auto" {
                                        auto = value == b"WBCorrection_Identity";
                                    }
                                }
                            )
                            .unwrap()
                    );
                    assert_eq!(links, 8);
                    assert!(auto);
                }

                if let Some(matrix) = entry.matrix().unwrap() {
                    matrices += 1;
                    if entry.name == b"DarkDrift" {
                        let top = matrix.dark_drift(0, 1531).unwrap();
                        assert_eq!(top[0], [-0.31608027f32, -5.762016]);
                        matrix.dark_drift(765, 1531).unwrap();
                        matrix.dark_drift(1530, 1531).unwrap();
                    }

                    if entry.name == b"PostPolyMatrix" {
                        assert_eq!(matrix.post_polynomial([0; 3]).unwrap(), [0; 3]);
                        matrix.post_polynomial([1000, 2000, 3000]).unwrap();
                    }

                    if entry.name == b"SpatialGain" {
                        assert_eq!(matrix.spatial_gain(0.0, 0.0).unwrap(), [1.0059f32, 1.0, 0.99091]);
                        for (x, y) in [(0.5, 0.5), (1.0, 1.0)] {
                            assert!(matrix.spatial_gain(x, y).unwrap().iter().all(|v| *v > 0.0));
                        }
                    }

                    if entry.name == b"AutoRGBNeutral" {
                        assert_eq!((matrix.element_type, matrix.elements), (3, 3));
                        assert_eq!(matrix.axis_size(0), Some(3));
                        assert_eq!(matrix.axis_size(1), None);
                        for (i, expected) in [0.8769354f32, 1.0, 0.9371507].into_iter().enumerate() {
                            assert_eq!(matrix.float32(i).unwrap(), expected);
                        }

                        assert!(
                            matrix
                                .data
                                .chunks_exact(4)
                                .all(|b| f32::from_le_bytes(b.try_into().unwrap()).is_finite())
                        );
                    }
                }
            },
        )
        .unwrap();
        assert_eq!(matrices, 47);
        assert_eq!(entries, 54);
        assert!(visit_x3f_camf(&calibration, || true, |_| {}).is_err());
        let mut malformed = calibration.to_vec();
        malformed[8..12].copy_from_slice(&0u32.to_le_bytes());
        assert!(visit_x3f_camf(&malformed, || false, |_| {}).is_err());

        eprintln!("SD10 CAMF type2 hash={}", blake3::hash(&calibration));
        drop(calibration);
        assert_eq!(calibration_budget.used(), 0);

        for end in 0..=28 {
            assert!(
                X3fEntry {
                    bytes: &entry.bytes[..end],
                    ..entry
                }
                .camf()
                .is_err()
            );
        }
        let sensor = inventory.entries().next().unwrap();
        let layout = sensor.legacy_huffman(|| false).unwrap();
        assert_eq!(layout.row(0).unwrap().len(), 3468);
        assert_eq!(layout.row(1530).unwrap().len(), 3468);
        assert!(layout.row(1531).is_none());
        assert!(layout.symbol(1024).is_none());
        let budget = rrrah_core::MemoryBudget::new(32 * 1024 * 1024);
        let channels = layout.decode_channels(&budget, || false).unwrap();

        assert_eq!(channels.pixels.len(), 2304 * 1531 * 3);
        let camf_entry = inventory.entries().find(|e| &e.kind == b"CAMF").unwrap();
        let calibration_budget = rrrah_core::MemoryBudget::new(65536);
        let calibration = camf_entry
            .camf()
            .unwrap()
            .unwrap()
            .decode_type2(&calibration_budget, || false)
            .unwrap();
        let mut drift = None;
        visit_x3f_camf(
            &calibration,
            || false,
            |entry| {
                if entry.name == b"DarkDrift" {
                    drift = entry.matrix().unwrap();
                }
            },
        )
        .unwrap();
        let drift = drift.unwrap();
        let row_budget = rrrah_core::MemoryBudget::new(65536);
        let rows = channels
            .black_rows([[6, 14], [2292, 2300]], 0.8, &drift, &row_budget, || false)
            .unwrap();
        assert_eq!(rows.len(), 1531 * 3);
        let black_oracle =
            std::fs::read(std::env::var_os("RRRAH_X3F_BLACK_ORACLE").expect("RRRAH_X3F_BLACK_ORACLE"))
                .unwrap();
        assert_eq!(black_oracle.len(), rows.len() * 4);
        let mut max_error = 0f32;
        for (i, (actual, bytes)) in rows.iter().zip(black_oracle.chunks_exact(4)).enumerate() {
            let expected = f32::from_le_bytes(bytes.try_into().unwrap());
            assert!(expected.is_finite());
            let error = (*actual - expected).abs();
            max_error = max_error.max(error);
            assert!(error <= 0.00005, "black row/channel {i}: {actual} vs {expected}");
        }
        eprintln!("SD10 initial black rows max error={max_error}");

        assert!(rows.iter().all(|v| v.is_finite()));
        let mut smoothed = rows.to_vec();
        let forward_mean = smooth_x3f_black_rows(&mut smoothed, || false).unwrap();
        let oracle =
            std::fs::read(std::env::var_os("RRRAH_X3F_SMOOTH_ORACLE").expect("RRRAH_X3F_SMOOTH_ORACLE"))
                .unwrap();
        assert_eq!(oracle.len(), smoothed.len() * 4);
        let mut max_error = 0f32;
        for (i, (actual, bytes)) in smoothed.iter().zip(oracle.chunks_exact(4)).enumerate() {
            let expected = f32::from_le_bytes(bytes.try_into().unwrap());
            let error = (*actual - expected).abs();
            max_error = max_error.max(error);
            assert!(
                expected.is_finite() && error <= 0.00005,
                "smoothed {i}: {actual} vs {expected}"
            );
        }
        eprintln!("SD10 smoothed black max error={max_error}");
        channels
            .adjust_black_scene(&mut smoothed, forward_mean, || false)
            .unwrap();
        let oracle =
            std::fs::read(std::env::var_os("RRRAH_X3F_SCENE_ORACLE").expect("RRRAH_X3F_SCENE_ORACLE"))
                .unwrap();
        assert_eq!(oracle.len(), smoothed.len() * 4);
        let mut max_error = 0f32;
        for (i, (actual, bytes)) in smoothed.iter().zip(oracle.chunks_exact(4)).enumerate() {
            let expected = f32::from_le_bytes(bytes.try_into().unwrap());
            let error = (*actual - expected).abs();
            max_error = max_error.max(error);
            assert!(
                expected.is_finite() && error <= 0.00005,
                "scene black {i}: {actual} vs {expected}"
            );
        }
        eprintln!("SD10 scene black max error={max_error}");
        let managed = channels
            .processed_black_rows([[6, 14], [2292, 2300]], 0.8, &drift, &row_budget, || false)
            .unwrap();
        assert_eq!(&*managed, &smoothed[..]);
        let resolved_budget = rrrah_core::MemoryBudget::new(65536);
        let resolved = channels
            .processed_black_from_camf(&calibration, &resolved_budget, || false)
            .unwrap();
        assert_eq!(&*resolved, &smoothed[..]);
        drop(resolved);
        assert_eq!(resolved_budget.used(), 0);
        let sensor_budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        // Separate arithmetic fixtures historically used shifted samples as explicit inputs.
        // Mark those inputs offset-free; whole-converter checks retain the real offset.
        let stage_channels = X3fLegacyChannels {
            width: channels.width,
            height: channels.height,
            legacy_offset: 0,
            pixels: channels.pixels.clone(),
        };
        if let Some(path) = std::env::var_os("RRRAH_X3F_LEGACY_FRAME_ORACLE") {
            let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
            let frame = stage_channels
                .process_auto(
                    &calibration,
                    [
                        [1.4032, -0.2231, -0.1016],
                        [-0.5263, 1.4816, 0.017],
                        [-0.0112, 0.0183, 0.9113],
                    ],
                    [0, 0, channels.width, channels.height],
                    &budget,
                    || false,
                )
                .unwrap();
            let expected = std::fs::read(path).unwrap();
            assert_eq!(expected.len(), frame.len() * 4);
            for (i, (actual, bytes)) in frame.iter().zip(expected.chunks_exact(4)).enumerate() {
                assert_eq!(
                    *actual,
                    i32::from_le_bytes(bytes.try_into().unwrap()),
                    "legacy frame channel {i}"
                );
            }
            assert_eq!(budget.used(), frame.len() as u64 * 4);
            drop(frame);
            assert_eq!(budget.used(), 0);
            if let Some(path) = std::env::var_os("RRRAH_X3F_FULL_CONVERTER_ORACLE") {
                let complete = channels
                    .process_auto(
                        &calibration,
                        [
                            [1.4032, -0.2231, -0.1016],
                            [-0.5263, 1.4816, 0.017],
                            [-0.0112, 0.0183, 0.9113],
                        ],
                        [0, 0, channels.width, channels.height],
                        &budget,
                        || false,
                    )
                    .unwrap();
                let expected = std::fs::read(path).unwrap();
                assert_eq!(expected.len(), complete.len() * 4);
                if let Some(path) = std::env::var_os("RRRAH_X3F_FULL_CORRECTED_ORACLE") {
                    let normalized = channels.unshifted(&budget, || false).unwrap();
                    let corrected = normalized
                        .correct_sensor_auto(&calibration, &budget, || false)
                        .unwrap();
                    let reference = std::fs::read(path).unwrap();
                    assert_eq!(reference.len(), corrected.len() * 4);
                    let mut n = 0usize;
                    let mut maximum = 0i64;
                    for (actual, bytes) in corrected.iter().zip(reference.chunks_exact(4)) {
                        let value = i32::from_le_bytes(bytes.try_into().unwrap());
                        if *actual != value {
                            n += 1;
                            maximum = maximum.max((i64::from(*actual) - i64::from(value)).abs());
                        }
                    }
                    eprintln!(
                        "SD10 full-converter sensor correction mismatches={n} max difference={maximum}"
                    );
                }
                let mut differences = 0usize;
                let mut maximum = 0i64;
                for (actual, bytes) in complete.iter().zip(expected.chunks_exact(4)) {
                    let other = i32::from_le_bytes(bytes.try_into().unwrap());
                    if *actual != other {
                        differences += 1;
                        maximum = maximum.max((i64::from(*actual) - i64::from(other)).abs());
                    }
                }
                eprintln!(
                    "SD10 full independent converter mismatches={differences} max difference={maximum}"
                );
                assert_eq!(differences, 0);
                if let Some(path) = std::env::var_os("RRRAH_X3F_FULL_CROP_ORACLE") {
                    let cropped = crop_x3f_channels(
                        &complete,
                        channels.width,
                        channels.height,
                        [18, 8, 2285, 1521],
                        &budget,
                        || false,
                    )
                    .unwrap();
                    let reference = std::fs::read(path).unwrap();
                    assert_eq!(cropped.len(), 2267 * 1513 * 3);
                    assert_eq!(reference.len(), cropped.len() * 4);
                    for (i, (actual, bytes)) in cropped.iter().zip(reference.chunks_exact(4)).enumerate() {
                        assert_eq!(
                            *actual,
                            i32::from_le_bytes(bytes.try_into().unwrap()),
                            "full converter crop channel {i}"
                        );
                    }
                    if let Some(path) = std::env::var_os("RRRAH_X3F_LINEAR_PPM_ORACLE") {
                        let ppm = std::fs::read(path).unwrap();
                        let header = b"P6\n2267 1513\n65535\n";
                        assert!(ppm.starts_with(header));
                        let payload = &ppm[header.len()..];
                        assert_eq!(payload.len(), cropped.len() * 2);
                        for (i, (value, bytes)) in cropped.iter().zip(payload.chunks_exact(2)).enumerate() {
                            assert_eq!(
                                *value,
                                i32::from(u16::from_be_bytes(bytes.try_into().unwrap())),
                                "linear PPM channel {i}"
                            );
                        }
                        eprintln!("SD10 native linear output exact against full converter 16-bit PPM");
                    }
                    if let Some(path) = std::env::var_os("RRRAH_X3F_LINEAR_SRGB_ORACLE") {
                        let output = x3f_linear_output(
                            &cropped,
                            [
                                [1.4032, -0.2231, -0.1016],
                                [-0.5263, 1.4816, 0.017],
                                [-0.0112, 0.0183, 0.9113],
                            ],
                            &budget,
                            || false,
                        )
                        .unwrap();
                        let ppm = std::fs::read(path).unwrap();
                        let header = b"P6\n2267 1513\n65535\n";
                        assert!(ppm.starts_with(header));
                        let payload = &ppm[header.len()..];
                        assert_eq!(payload.len(), output.len() * 2);
                        for (i, (actual, bytes)) in output.iter().zip(payload.chunks_exact(2)).enumerate() {
                            assert_eq!(
                                *actual,
                                u16::from_be_bytes(bytes.try_into().unwrap()),
                                "linear sRGB channel {i}"
                            );
                        }
                        let transport_budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
                        let raster =
                            x3f_linear_raster(&output, 2267, 1513, &transport_budget, || false).unwrap();
                        let linear = raster
                            .to_linear_srgb_with_budget_and_cancel(Some(&transport_budget), || false)
                            .unwrap();
                        assert_eq!(linear.color_space(), &rrrah_core::RasterColorSpace::LinearSrgb);
                        let rrrah_core::RasterPixels::Rgba32Float(values) = linear.pixels() else {
                            panic!("GPU transport precision")
                        };
                        for (i, (source, rgba)) in
                            output.chunks_exact(3).zip(values.chunks_exact(4)).enumerate()
                        {
                            for c in 0..3 {
                                assert_eq!(
                                    rgba[c],
                                    f32::from(source[c]) / 65535.0,
                                    "transport pixel {i} channel {c}"
                                );
                            }
                            assert_eq!(rgba[3], 1.0);
                        }
                        drop(linear);
                        drop(raster);
                        assert_eq!(transport_budget.used(), 0);
                        eprintln!("SD10 raster linear float transport exact; all leases released");
                        drop(output);
                        eprintln!("SD10 linear sRGB output exact against full converter PPM");
                    }
                    drop(cropped);
                    eprintln!("SD10 full converter crop exact 2267x1513");
                }
                drop(complete);
                assert_eq!(budget.used(), 0);
            }
            eprintln!("SD10 managed legacy Foveon target full frame exact");
        }
        let corrected = channels
            .correct_sensor_channels(&calibration, [0.8769354, 1.0, 0.9371507], &sensor_budget, || {
                false
            })
            .unwrap();
        assert_eq!(corrected.len(), channels.pixels.len());
        if let Some(path) = std::env::var_os("RRRAH_X3F_REPAIRED_ORACLE") {
            let mut repaired = corrected.to_vec();
            let count = repair_x3f_bad_pixels_from_camf(
                &mut repaired,
                channels.width,
                channels.height,
                &calibration,
                || false,
            )
            .unwrap();
            let oracle = std::fs::read(path).unwrap();
            assert_eq!(oracle.len(), repaired.len() * 4);
            for (i, (actual, bytes)) in repaired.iter().zip(oracle.chunks_exact(4)).enumerate() {
                assert_eq!(
                    *actual,
                    i32::from_le_bytes(bytes.try_into().unwrap()),
                    "repaired channel {i}"
                );
            }
            eprintln!("SD10 repaired channels exact; repaired pixels={count}");
            if let Some(path) = std::env::var_os("RRRAH_X3F_RED_ORACLE") {
                let scratch = rrrah_core::MemoryBudget::new(1024 * 1024);
                sharpen_x3f_red(&mut repaired, channels.width, channels.height, &scratch, || false).unwrap();
                assert_eq!(scratch.used(), 0);
                let oracle = std::fs::read(path).unwrap();
                assert_eq!(oracle.len(), repaired.len() * 4);
                for (i, (actual, bytes)) in repaired.iter().zip(oracle.chunks_exact(4)).enumerate() {
                    assert_eq!(
                        *actual,
                        i32::from_le_bytes(bytes.try_into().unwrap()),
                        "red stage channel {i}"
                    );
                }
                eprintln!("SD10 red sharpened channels exact");
                if let Some(path) = std::env::var_os("RRRAH_X3F_HIGHLIGHT_ORACLE") {
                    linearize_x3f_highlights_from_camf(&mut repaired, &calibration, || false).unwrap();
                    let oracle = std::fs::read(path).unwrap();
                    assert_eq!(oracle.len(), repaired.len() * 4);
                    for (i, (actual, bytes)) in repaired.iter().zip(oracle.chunks_exact(4)).enumerate() {
                        assert_eq!(
                            *actual,
                            i32::from_le_bytes(bytes.try_into().unwrap()),
                            "highlight channel {i}"
                        );
                    }
                    eprintln!("SD10 highlight channels exact");
                    if let Some(path) = std::env::var_os("RRRAH_X3F_DERIVED_FRAME_ORACLE") {
                        let budget = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
                        let transform = x3f_color_transform_auto(&calibration, identity, || false).unwrap();
                        let curves =
                            x3f_noise_curves_auto(&calibration, transform.luminance, &budget, || false)
                                .unwrap();
                        let mut frame = repaired.clone();
                        smooth_x3f_hues(
                            &mut frame,
                            channels.width,
                            channels.height,
                            &curves[7],
                            &budget,
                            || false,
                        )
                        .unwrap();
                        smooth_x3f_hues_wide(
                            &mut frame,
                            channels.width,
                            channels.height,
                            &curves[6],
                            &budget,
                            || false,
                        )
                        .unwrap();
                        transform_x3f_pixels(
                            &mut frame,
                            [&curves[0], &curves[1], &curves[2]],
                            &transform,
                            || false,
                        )
                        .unwrap();
                        smooth_x3f_chroma(
                            &mut frame,
                            channels.width,
                            channels.height,
                            [&curves[3], &curves[4], &curves[5]],
                            &budget,
                            || false,
                        )
                        .unwrap();
                        drop(curves);
                        assert_eq!(budget.used(), 0);
                        let expected = std::fs::read(path).unwrap();
                        assert_eq!(expected.len(), frame.len() * 4);
                        for (i, (actual, bytes)) in frame.iter().zip(expected.chunks_exact(4)).enumerate() {
                            assert_eq!(
                                *actual,
                                i32::from_le_bytes(bytes.try_into().unwrap()),
                                "derived frame channel {i}"
                            );
                        }
                        eprintln!(
                            "SD10 complete derived-luminance filter chain exact for explicit identity target"
                        );
                        let managed_budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
                        let managed = stage_channels
                            .process_auto(
                                &calibration,
                                identity,
                                [0, 0, channels.width, channels.height],
                                &managed_budget,
                                || false,
                            )
                            .unwrap();
                        assert_eq!(&managed[..], &frame[..]);
                        assert_eq!(managed_budget.used(), managed.len() as u64 * 4);
                        drop(managed);
                        assert_eq!(managed_budget.used(), 0);
                        assert!(
                            channels
                                .process_auto(
                                    &calibration,
                                    identity,
                                    [0, 0, channels.width, channels.height],
                                    &managed_budget,
                                    || true
                                )
                                .is_err()
                        );
                        assert_eq!(managed_budget.used(), 0);
                        eprintln!("SD10 managed complete processing exact; output lease released");
                        let frame_bytes = channels.pixels.len() as u64 * 4;
                        for limit in [0, frame_bytes, frame_bytes + 32768] {
                            let constrained = rrrah_core::MemoryBudget::new(limit);
                            assert!(matches!(
                                channels.process_auto(
                                    &calibration,
                                    identity,
                                    [0, 0, channels.width, channels.height],
                                    &constrained,
                                    || false
                                ),
                                Err(X3fError::Memory(_))
                            ));
                            assert_eq!(constrained.used(), 0, "failed admission leaked limit {limit}");
                        }
                        let mut observed_allocation = false;
                        let result = channels.process_auto(
                            &calibration,
                            identity,
                            [0, 0, channels.width, channels.height],
                            &managed_budget,
                            || {
                                if managed_budget.used() > 0 {
                                    observed_allocation = true;
                                    true
                                } else {
                                    false
                                }
                            },
                        );
                        assert!(observed_allocation);
                        assert!(matches!(result, Err(X3fError::Cancelled)));
                        assert_eq!(managed_budget.used(), 0);
                        eprintln!(
                            "SD10 managed processing budget failures and post-allocation cancellation release to zero"
                        );
                    }

                    if let Some(path) = std::env::var_os("RRRAH_X3F_HUE_ORACLE") {
                        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
                        let curves = x3f_noise_curves_auto(&calibration, 1.0, &budget, || false).unwrap();
                        let held = budget.used();
                        smooth_x3f_hues(
                            &mut repaired,
                            channels.width,
                            channels.height,
                            &curves[7],
                            &budget,
                            || false,
                        )
                        .unwrap();
                        assert_eq!(budget.used(), held);
                        drop(curves);
                        assert_eq!(budget.used(), 0);
                        let oracle = std::fs::read(path).unwrap();
                        assert_eq!(oracle.len(), repaired.len() * 4);
                        for (i, (actual, bytes)) in repaired.iter().zip(oracle.chunks_exact(4)).enumerate() {
                            assert_eq!(
                                *actual,
                                i32::from_le_bytes(bytes.try_into().unwrap()),
                                "hue channel {i}"
                            );
                        }
                        eprintln!("SD10 first hue pass channels exact at explicit luminance=1");
                        if let Some(path) = std::env::var_os("RRRAH_X3F_WIDE_HUE_ORACLE") {
                            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
                            let curves = x3f_noise_curves_auto(&calibration, 1.0, &budget, || false).unwrap();
                            smooth_x3f_hues_wide(
                                &mut repaired,
                                channels.width,
                                channels.height,
                                &curves[6],
                                &budget,
                                || false,
                            )
                            .unwrap();
                            drop(curves);
                            assert_eq!(budget.used(), 0);
                            let oracle = std::fs::read(path).unwrap();
                            assert_eq!(oracle.len(), repaired.len() * 4);
                            for (i, (actual, bytes)) in
                                repaired.iter().zip(oracle.chunks_exact(4)).enumerate()
                            {
                                assert_eq!(
                                    *actual,
                                    i32::from_le_bytes(bytes.try_into().unwrap()),
                                    "wide hue channel {i}"
                                );
                            }
                            eprintln!("SD10 second hue pass channels exact at explicit luminance=1");
                            if let Some(path) = std::env::var_os("RRRAH_X3F_COLOR_ORACLE") {
                                let curves =
                                    x3f_noise_curves_auto(&calibration, 1.0, &budget, || false).unwrap();
                                let transform = x3f_color_transform_auto(
                                    &calibration,
                                    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                                    || false,
                                )
                                .unwrap();
                                transform_x3f_pixels(
                                    &mut repaired,
                                    [&curves[0], &curves[1], &curves[2]],
                                    &transform,
                                    || false,
                                )
                                .unwrap();
                                drop(curves);
                                assert_eq!(budget.used(), 0);
                                let expected = std::fs::read(path).unwrap();
                                assert_eq!(expected.len(), repaired.len() * 4);
                                for (index, (actual, bytes)) in
                                    repaired.iter().zip(expected.chunks_exact(4)).enumerate()
                                {
                                    assert_eq!(
                                        *actual,
                                        i32::from_le_bytes(bytes.try_into().unwrap()),
                                        "color channel {index}"
                                    );
                                }
                                eprintln!(
                                    "SD10 pixel transform exact for explicit identity target and luminance=1 curves"
                                );
                                if let Some(path) = std::env::var_os("RRRAH_X3F_CHROMA_ORACLE") {
                                    let budget = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
                                    let guide = x3f_chroma_guide(
                                        &repaired,
                                        channels.width,
                                        channels.height,
                                        &budget,
                                        || false,
                                    )
                                    .unwrap();
                                    let expected =
                                        std::fs::read(std::env::var_os("RRRAH_X3F_GUIDE_ORACLE").unwrap())
                                            .unwrap();
                                    assert_eq!(expected.len(), guide.len() * 2);
                                    for (i, (actual, bytes)) in
                                        guide.iter().zip(expected.chunks_exact(2)).enumerate()
                                    {
                                        assert_eq!(
                                            *actual,
                                            i16::from_le_bytes(bytes.try_into().unwrap()),
                                            "guide channel {i}"
                                        );
                                    }
                                    drop(guide);
                                    let curves =
                                        x3f_noise_curves_auto(&calibration, 1.0, &budget, || false).unwrap();
                                    smooth_x3f_chroma(
                                        &mut repaired,
                                        channels.width,
                                        channels.height,
                                        [&curves[3], &curves[4], &curves[5]],
                                        &budget,
                                        || false,
                                    )
                                    .unwrap();
                                    drop(curves);
                                    assert_eq!(budget.used(), 0);
                                    let expected = std::fs::read(path).unwrap();
                                    assert_eq!(expected.len(), repaired.len() * 4);
                                    for (i, (actual, bytes)) in
                                        repaired.iter().zip(expected.chunks_exact(4)).enumerate()
                                    {
                                        assert_eq!(
                                            *actual,
                                            i32::from_le_bytes(bytes.try_into().unwrap()),
                                            "chroma channel {i}"
                                        );
                                    }
                                    eprintln!(
                                        "SD10 final chroma and quarter guide exact at explicit test target/curves"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(path) = std::env::var_os("RRRAH_X3F_CORRECTED_ORACLE") {
            let oracle = std::fs::read(path).unwrap();
            assert_eq!(oracle.len(), corrected.len() * 4);
            let mut mismatches = 0usize;
            let mut maximum = 0i64;
            for (actual, bytes) in corrected.iter().zip(oracle.chunks_exact(4)) {
                let expected = i32::from_le_bytes(bytes.try_into().unwrap());
                if *actual != expected {
                    mismatches += 1;
                    maximum = maximum.max((i64::from(*actual) - i64::from(expected)).abs());
                }
            }
            eprintln!("SD10 corrected channels mismatches={mismatches} max difference={maximum}");
            assert_eq!(
                mismatches, 0,
                "Independent corrected-channel equality remains required"
            );
        }

        assert!(corrected.iter().all(|v| *v <= 32000));
        assert_eq!(sensor_budget.used(), corrected.capacity_bytes());
        let resolved_budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let resolved = channels
            .correct_sensor_auto(&calibration, &resolved_budget, || false)
            .unwrap();
        assert_eq!(&*resolved, &*corrected);
        drop(resolved);
        assert_eq!(resolved_budget.used(), 0);
        assert!(matches!(
            channels.correct_sensor_auto(&calibration, &resolved_budget, || true),
            Err(X3fError::Cancelled)
        ));
        let mut missing = calibration.to_vec();
        // Remove the embedded neutral and invalidate its calibrated derivation links.
        for index in 0..missing.len().saturating_sub(14) {
            if &missing[index..index + 14] == b"AutoRGBNeutral" {
                missing[index] = b'X';
            }
        }
        for index in 0..missing.len().saturating_sub(23) {
            if &missing[index..index + 23] == b"WhiteBalanceIlluminants" { missing[index] = b'X'; }
        }
        assert!(
            channels
                .correct_sensor_auto(&missing, &resolved_budget, || false)
                .is_err()
        );
        assert_eq!(resolved_budget.used(), 0);
        drop(corrected);
        assert_eq!(sensor_budget.used(), 0);
        assert!(
            channels
                .correct_sensor_channels(&calibration, [0.0; 3], &sensor_budget, || false)
                .is_err()
        );
        let mut calls = 0;
        assert!(matches!(
            channels.correct_sensor_channels(&calibration, [1.0; 3], &sensor_budget, || {
                calls += 1;
                calls == 10000
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(sensor_budget.used(), 0);
        let mut ambiguous = calibration.to_vec();
        ambiguous.extend_from_slice(&calibration);
        assert!(
            channels
                .processed_black_from_camf(&ambiguous, &resolved_budget, || false)
                .is_err()
        );
        assert_eq!(resolved_budget.used(), 0);
        assert!(matches!(
            channels.processed_black_from_camf(&calibration, &resolved_budget, || true),
            Err(X3fError::Cancelled)
        ));

        let retained = managed.clone();
        drop(managed);
        assert_eq!(
            row_budget.used(),
            retained.capacity_bytes() + rows.capacity_bytes()
        );
        drop(retained);
        let tiny = rrrah_core::MemoryBudget::new(rows.capacity_bytes());
        assert!(matches!(
            channels.processed_black_rows([[6, 14], [2292, 2300]], 0.8, &drift, &tiny, || false),
            Err(X3fError::Memory(_))
        ));
        assert_eq!(tiny.used(), 0);
        let mut checks = 0;
        assert!(matches!(
            channels.processed_black_rows([[6, 14], [2292, 2300]], 0.8, &drift, &row_budget, || {
                checks += 1;
                checks == 10000
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(row_budget.used(), rows.capacity_bytes());

        drop(rows);
        assert_eq!(row_budget.used(), 0);
        assert!(matches!(
            channels.black_rows(
                [[6, 14], [2292, 2300]],
                0.8,
                &drift,
                &rrrah_core::MemoryBudget::new(0),
                || false
            ),
            Err(X3fError::Memory(_))
        ));
        let mut calls = 0;
        assert!(matches!(
            channels.black_rows([[6, 14], [2292, 2300]], 0.8, &drift, &row_budget, || {
                calls += 1;
                calls == 8
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(row_budget.used(), 0);
        drop(drift);
        drop(calibration);
        assert_eq!(calibration_budget.used(), 0);

        assert!(
            channels
                .shield_mean([10, 2, 2294, 8], || false)
                .unwrap()
                .iter()
                .all(|v| v.is_finite())
        );
        // CAMF bottom shield exceeds this decoded frame: refuse until mapping is qualified.
        assert!(channels.shield_mean([10, 1528, 2294, 1534], || false).is_err());

        let oracle = std::fs::read(std::env::var_os("RRRAH_X3F_ORACLE").expect("RRRAH_X3F_ORACLE")).unwrap();
        assert_eq!(oracle.len(), channels.pixels.len() * 2);
        for (index, (actual, expected)) in channels.pixels.iter().zip(oracle.chunks_exact(2)).enumerate() {
            assert_eq!(
                *actual,
                u16::from_le_bytes([expected[0], expected[1]]),
                "sensor sample {index}"
            );
        }
        assert_eq!(budget.used(), channels.pixels.capacity_bytes());
        eprintln!(
            "SD10 native legacy offset={} sensor hash={}",
            channels.legacy_offset,
            {
                let mut hash = blake3::Hasher::new();
                for sample in channels.pixels.iter() {
                    hash.update(&sample.to_le_bytes());
                }
                hash.finalize()
            }
        );
        drop(stage_channels);
        drop(channels);
        assert_eq!(budget.used(), 0);
        if let Some(path) = std::env::var_os("RRRAH_X3F_LINEAR_SRGB_ORACLE") {
            let budget = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
            let raster = decode_x3f_legacy_auto(&bytes, &budget, || false).unwrap();
            assert_eq!((raster.width(), raster.height()), (2267, 1513));
            let expected = std::fs::read(path).unwrap();
            let header = b"P6\n2267 1513\n65535\n";
            assert!(expected.starts_with(header));
            let rrrah_core::RasterPixels::Rgba16(values) = raster.pixels() else {
                panic!("precision")
            };
            assert_eq!(expected.len() - header.len(), values.len() / 4 * 6);
            for (source, pixel) in expected[header.len()..]
                .chunks_exact(6)
                .zip(values.chunks_exact(4))
            {
                for c in 0..3 {
                    assert_eq!(pixel[c], u16::from_be_bytes([source[c * 2], source[c * 2 + 1]]));
                }
                assert_eq!(pixel[3], 65535);
            }
            drop(raster);
            assert_eq!(budget.used(), 0);
            eprintln!("SD10 native container-to-linear-raster exact; all leases released");
            for (degrees, variable, dimensions) in [
                (90u32, "RRRAH_X3F_ROTATED_ORACLE", (1513, 2267)),
                (180, "RRRAH_X3F_ROTATED180_ORACLE", (2267, 1513)),
                (270, "RRRAH_X3F_ROTATED270_ORACLE", (1513, 2267)),
            ] {
                let Some(path) = std::env::var_os(variable) else { continue };
                let mut rotated_source = bytes.clone();
                rotated_source[36..40].copy_from_slice(&degrees.to_le_bytes());
                let rotated = decode_x3f_legacy_auto(&rotated_source, &budget, || false).unwrap();
                assert_eq!((rotated.width(), rotated.height()), dimensions);
                let expected = std::fs::read(path).unwrap();
                let header = format!("P6\n{} {}\n65535\n", dimensions.0, dimensions.1);
                assert!(expected.starts_with(header.as_bytes()));
                let rrrah_core::RasterPixels::Rgba16(values) = rotated.pixels() else {
                    panic!()
                };
                assert_eq!(expected.len() - header.len(), values.len() / 4 * 6);
                for (source, pixel) in expected[header.len()..]
                    .chunks_exact(6)
                    .zip(values.chunks_exact(4))
                {
                    for c in 0..3 {
                        assert_eq!(pixel[c], u16::from_be_bytes([source[c * 2], source[c * 2 + 1]]));
                    }
                    assert_eq!(pixel[3], 65535);
                }
                drop(rotated);
                assert_eq!(budget.used(), 0);
                eprintln!("SD10 header rotation {degrees} exact against independent full converter");
            }

            let path = std::env::var_os("RRRAH_X3F_SOURCE").unwrap();
            let mut request = crate::DecodeRequest::new(std::path::PathBuf::from(path));
            request.memory_budget = Some(budget.clone());
            assert_eq!(
                crate::image_source_kind(&request).unwrap(),
                crate::ImageSourceKind::Raster
            );
            let opened = crate::decode_raster(&request).unwrap();
            assert_eq!((opened.width(), opened.height()), (2267, 1513));
            assert_eq!(opened.color_space(), &rrrah_core::RasterColorSpace::LinearSrgb);
            drop(opened);
            assert_eq!(budget.used(), 0);
            request.image_index = 1;
            assert!(crate::decode_raster(&request).is_err());
            assert_eq!(budget.used(), 0);
            eprintln!("SD10 public raster opening and selection refusal pass");
        }
        let images: Vec<_> = inventory
            .entries()
            .filter_map(|entry| entry.image_info().unwrap())
            .collect();
        assert_eq!(
            images,
            [
                X3fImageInfo {
                    image_type: 3,
                    encoding: 6,
                    width: 2304,
                    height: 1531,
                    row_bytes: 0
                },
                X3fImageInfo {
                    image_type: 2,
                    encoding: 11,
                    width: 567,
                    height: 378,
                    row_bytes: 0
                },
                X3fImageInfo {
                    image_type: 2,
                    encoding: 3,
                    width: 189,
                    height: 126,
                    row_bytes: 568
                },
            ]
        );
    }
    #[test]
    fn image_header_refuses_truncation_and_reports_reserved_sensor_encoding() {
        let mut bytes = vec![0; 28];
        bytes[..4].copy_from_slice(b"SECi");
        for (offset, value) in [(4, 0x0002_0000u32), (8, 3), (12, 6), (16, 4), (20, 2)] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for end in 0..28 {
            assert!(
                X3fEntry {
                    kind: *b"IMAG",
                    offset: 0,
                    bytes: &bytes[..end]
                }
                .image_info()
                .is_err()
            );
        }
        assert_eq!(
            X3fEntry {
                kind: *b"IMAG",
                offset: 0,
                bytes: &bytes
            }
            .image_info()
            .unwrap()
            .unwrap()
            .encoding,
            6
        );
        bytes[24] = 1;
        assert!(
            X3fEntry {
                kind: *b"IMAG",
                offset: 0,
                bytes: &bytes
            }
            .image_info()
            .is_err()
        );
    }
    #[test]
    fn legacy_tables_refuse_prefix_collisions_row_aliasing_and_cancel() {
        let mut bytes = vec![0; 6172 + 2 + 8];
        bytes[..4].copy_from_slice(b"SECi");
        for (offset, value) in [
            (4, 0x0002_0000u32),
            (8, 3),
            (12, 6),
            (16, 1),
            (20, 2),
            (2076, 1 << 27),
            (2080, (1 << 27) | 1),
            (6178, 1),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        fn entry(b: &[u8]) -> X3fEntry<'_> {
            X3fEntry {
                kind: *b"IMAG",
                offset: 0,
                bytes: b,
            }
        }
        assert_eq!(
            entry(&bytes).legacy_huffman(|| false).unwrap().row(0),
            Some(&[0][..])
        );
        let mut wrapping = bytes.clone();
        wrapping[16..20].copy_from_slice(&2u32.to_le_bytes());
        wrapping[28..30].copy_from_slice(&(-32760i16).to_le_bytes());
        wrapping[30..32].copy_from_slice(&5i16.to_le_bytes());
        wrapping[6172..6174].copy_from_slice(&[0x5c,0x5c]);
        let budget = rrrah_core::MemoryBudget::new(4096);
        let layout = entry(&wrapping).legacy_huffman(|| false).unwrap();
        let result = layout.decode_channels(&budget, || false).unwrap();
        assert_eq!(result.legacy_offset,32760);
        assert_eq!(&*result.pixels,&[0,32765,0,5,0,5,0,32765,0,5,0,5]);
        drop(result); assert_eq!(budget.used(),0);
        wrapping[28..30].copy_from_slice(&i16::MIN.to_le_bytes());
        assert!(entry(&wrapping).legacy_huffman(|| false).unwrap().decode_channels(&budget,||false).is_err());
        assert_eq!(budget.used(),0);
        bytes[28..30].copy_from_slice(&(-1i16).to_le_bytes());
        let layout = entry(&bytes).legacy_huffman(|| false).unwrap();
        let budget = rrrah_core::MemoryBudget::new(4096);
        let channels = layout.decode_channels(&budget, || false).unwrap();
        assert_eq!(channels.legacy_offset, 1);
        assert_eq!(&*channels.pixels, &[0; 6]);
        drop(channels);
        assert_eq!(budget.used(), 0);
        assert!(matches!(
            layout.decode_channels(&rrrah_core::MemoryBudget::new(0), || false),
            Err(X3fError::Memory(_))
        ));
        let mut polls = 0;
        assert!(matches!(
            layout.decode_channels(&budget, || {
                polls += 1;
                polls == 3
            }),
            Err(X3fError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        let mut calls = 0;
        assert!(matches!(
            entry(&bytes).legacy_huffman(|| {
                calls += 1;
                calls == 3
            }),
            Err(X3fError::Cancelled)
        ));
        bytes[2080..2084].copy_from_slice(&(2u32 << 27).to_le_bytes());
        assert!(matches!(
            entry(&bytes).legacy_huffman(|| false),
            Err(X3fError::Invalid)
        ));
        bytes[2080..2084].copy_from_slice(&((1u32 << 27) | 1).to_le_bytes());
        bytes[6178..6182].fill(0);
        assert!(matches!(
            entry(&bytes).legacy_huffman(|| false),
            Err(X3fError::Invalid)
        ));
    }
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 48];
        bytes[..4].copy_from_slice(b"FOVb");
        for (offset, value) in [(4, 0x0002_0000u32), (28, 2), (32, 1), (36, 90)] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[40..48].copy_from_slice(b"unknown!");
        bytes.extend_from_slice(b"SECd");
        for value in [0x0002_0000u32, 1, 40, 8] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(b"TEST");
        bytes.extend_from_slice(&48u32.to_le_bytes());
        bytes
    }
    #[test]
    fn borrowed_unknown_section_and_all_truncated_prefixes() {
        let bytes = fixture();
        let inventory = inspect_x3f(&bytes, || false).unwrap();
        assert_eq!(
            (inventory.width, inventory.height, inventory.rotation_degrees),
            (2, 1, 90)
        );
        let entry = inventory.entries().next().unwrap();
        assert_eq!(entry.kind, *b"TEST");
        assert_eq!(entry.bytes, b"unknown!");
        assert_eq!(entry.bytes.as_ptr(), bytes[40..].as_ptr());
        for end in 0..bytes.len() {
            assert!(inspect_x3f(&bytes[..end], || false).is_err());
        }
        for version in [0x0002_0001u32, 0x0002_0002] {
            let mut extended = bytes.clone();
            extended.splice(40..40, [0; 192]);
            extended[4..8].copy_from_slice(&version.to_le_bytes());
            extended[252..256].copy_from_slice(&232u32.to_le_bytes());
            let end = extended.len();
            extended[end - 4..].copy_from_slice(&240u32.to_le_bytes());
            let inventory = inspect_x3f(&extended, || false).unwrap();
            assert_eq!(inventory.entries().next().unwrap().bytes, b"unknown!");
        }
    }
    #[test]
    fn invalid_ranges_versions_dimensions_and_cancellation() {
        let original = fixture();
        for (offset, value) in [
            (4, 0x0003_0000u32),
            (28, 0),
            (36, 45),
            (56, 1025),
            (60, 41),
            (64, u32::MAX),
        ] {
            let mut bytes = original.clone();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(inspect_x3f(&bytes, || false).is_err());
        }
        let mut calls = 0;
        assert!(matches!(
            inspect_x3f(&original, || {
                calls += 1;
                calls == 2
            }),
            Err(X3fError::Cancelled)
        ));
        let mut bytes = original;
        bytes.truncate(72);
        bytes[56..60].copy_from_slice(&2u32.to_le_bytes());
        for value in [40u32, 8] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(b"TEST");
        bytes.extend_from_slice(&48u32.to_le_bytes());
        assert!(matches!(inspect_x3f(&bytes, || false), Err(X3fError::Invalid)));
    }
}
