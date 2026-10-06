use image::{DynamicImage, GrayImage, ImageBuffer, Luma, Rgb, imageops};
use ndarray::{Array, Array2, Zip, s};
use std::cmp;
use std::collections::HashSet;
use std::f64::consts::PI;
use std::ffi::CString;
use std::fmt;
use std::os::raw::c_char;
use std::ptr;

fn hex_to_bool_vec(hex: &str) -> Vec<bool> {
    let mut bits = Vec::with_capacity(hex.len() * 4);
    for c in hex.chars() {
        if let Some(n) = c.to_digit(16) {
            for i in (0..4).rev() {
                bits.push((n >> i) & 1 == 1);
            }
        }
    }
    bits
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImageHash {
    pub data: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub bit_len: usize,
}

impl ImageHash {
    pub fn from_bool_array(arr: &Array2<bool>) -> Self {
        let (height, width) = arr.dim();
        let bit_len = arr.len();

        let mut data = Vec::with_capacity((bit_len + 7) / 8);
        let mut current_byte = 0u8;
        let mut bit_idx = 0;

        for &b in arr.iter() {
            if b {
                current_byte |= 1 << (7 - bit_idx);
            }
            bit_idx += 1;

            if bit_idx == 8 {
                data.push(current_byte);
                current_byte = 0;
                bit_idx = 0;
            }
        }

        if bit_idx > 0 {
            data.push(current_byte);
        }

        ImageHash { data, width, height, bit_len }
    }

    pub fn distance(&self, other: &ImageHash) -> u32 {
        if self.width != other.width || self.height != other.height {
            return u32::MAX;
        }

        self.data
            .iter()
            .zip(other.data.iter())
            .map(|(&a, &b)| (a ^ b).count_ones())
            .sum()
    }

    pub fn len(&self) -> usize {
        self.bit_len
    }

    pub fn is_empty(&self) -> bool {
        self.bit_len == 0
    }
}

impl fmt::Display for ImageHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.data {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

pub fn hex_to_hash(hexstr: &str) -> ImageHash {
    let byte_len = (hexstr.len() + 1) / 2;
    let mut data = Vec::with_capacity(byte_len);

    for i in 0..byte_len {
        let end = std::cmp::min((i + 1) * 2, hexstr.len());
        let byte_str = &hexstr[i * 2 .. end];
        if let Ok(byte) = u8::from_str_radix(byte_str, 16) {
            data.push(byte);
        } else {
            return ImageHash { data: vec![], width: 0, height: 0, bit_len: 0 };
        }
    }

    let bit_len = hexstr.len() * 4;
    let hash_size = (bit_len as f64).sqrt() as usize;

    ImageHash {
        data,
        width: hash_size,
        height: hash_size,
        bit_len
    }
}

pub fn hex_to_flathash(hexstr: &str, hashsize: usize) -> ImageHash {
    if hashsize == 0 {
        return ImageHash { data: vec![], width: 0, height: 0, bit_len: 0 };
    }
    let bits = hex_to_bool_vec(hexstr);
    let hash_size = bits.len() / hashsize;
    let total_bits = hash_size * hashsize;
    if bits.len() < total_bits {
        return ImageHash { data: vec![], width: 0, height: 0, bit_len: 0 };
    }
    let bits = &bits[bits.len() - total_bits..];
    let arr = Array2::from_shape_vec((hashsize, hash_size), bits.to_vec())
        .unwrap_or_else(|_| Array2::from_elem((0, 0), false));
    ImageHash::from_bool_array(&arr)
}

pub fn hex_to_multihash(hexstr: &str) -> ImageMultiHash {
    let hashes: Vec<ImageHash> = hexstr.split(',').map(|s| hex_to_hash(s)).collect();
    ImageMultiHash::new(hashes)
}

pub fn old_hex_to_hash(hexstr: &str, hash_size: usize) -> ImageHash {
    let count = hash_size * (hash_size / 4);
    if hexstr.len() != count || count == 0 {
        return ImageHash { data: vec![], width: 0, height: 0, bit_len: 0 };
    }
    let mut arr = Vec::new();
    for i in 0..(count / 2) {
        if let Some(hex_byte) = hexstr.get(i * 2..i * 2 + 2) {
            if let Ok(v) = u8::from_str_radix(hex_byte, 16) {
                let bits: Vec<bool> = (0..8).map(|bit| (v >> bit) & 1 == 1).collect();
                arr.push(bits);
            }
        }
    }
    if arr.is_empty() {
        return ImageHash { data: vec![], width: 0, height: 0, bit_len: 0 };
    }
    let shape = (arr.len(), arr[0].len());
    let flat: Vec<bool> = arr.into_iter().flatten().collect();
    let hash_array = Array2::from_shape_vec(shape, flat)
        .unwrap_or_else(|_| Array2::from_elem((0, 0), false));
    ImageHash::from_bool_array(&hash_array)
}

#[derive(Clone, Debug)]
pub struct ImageMultiHash {
    pub segment_hashes: Vec<ImageHash>,
}

impl ImageMultiHash {
    pub fn new(hashes: Vec<ImageHash>) -> Self {
        ImageMultiHash {
            segment_hashes: hashes,
        }
    }

    pub fn hash_diff(
        &self,
        other: &ImageMultiHash,
        hamming_cutoff: Option<f64>,
        bit_error_rate: Option<f64>,
    ) -> (usize, u32) {
        if self.segment_hashes.is_empty() || other.segment_hashes.is_empty() {
            return (0, 0);
        }
        let bit_error_rate = bit_error_rate.unwrap_or(0.25);
        let hamming_cutoff = hamming_cutoff
            .unwrap_or_else(|| (self.segment_hashes[0].len() as f64 * bit_error_rate).ceil());

        let mut distances = Vec::new();
        for seg_hash in &self.segment_hashes {
            let lowest = other
                .segment_hashes
                .iter()
                .map(|other_hash| seg_hash.distance(other_hash))
                .min()
                .unwrap_or(u32::MAX);
            if lowest <= hamming_cutoff as u32 {
                distances.push(lowest);
            }
        }
        (distances.len(), distances.iter().sum())
    }

    pub fn matches(
        &self,
        other: &ImageMultiHash,
        region_cutoff: usize,
        hamming_cutoff: Option<f64>,
        bit_error_rate: Option<f64>,
    ) -> bool {
        let (matches, _) = self.hash_diff(other, hamming_cutoff, bit_error_rate);
        matches >= region_cutoff
    }

    pub fn distance_score(
        &self,
        other: &ImageMultiHash,
        hamming_cutoff: Option<f64>,
        bit_error_rate: Option<f64>,
    ) -> f64 {
        let max_difference = self.segment_hashes.len() as f64;
        if self.segment_hashes.is_empty() {
            return max_difference;
        }
        let (matches, sum_dist) = self.hash_diff(other, hamming_cutoff, bit_error_rate);
        if matches == 0 {
            return max_difference;
        }
        let max_distance = matches as f64 * self.segment_hashes[0].len() as f64;
        let tie_breaker = (sum_dist as f64) / max_distance;
        let match_score = matches as f64 - tie_breaker;
        max_difference - match_score
    }

    pub fn best_match(
        &self,
        others: &[ImageMultiHash],
        hamming_cutoff: Option<f64>,
        bit_error_rate: Option<f64>,
    ) -> ImageMultiHash {
        if others.is_empty() {
            return self.clone();
        }
        others
            .iter()
            .min_by(|a, b| {
                self.distance_score(a, hamming_cutoff, bit_error_rate)
                    .partial_cmp(&self.distance_score(b, hamming_cutoff, bit_error_rate))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap()
            .clone()
    }
}

impl fmt::Display for ImageMultiHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hashes: Vec<String> = self.segment_hashes.iter().map(|h| h.to_string()).collect();
        write!(f, "{}", hashes.join(","))
    }
}

impl PartialEq for ImageMultiHash {
    fn eq(&self, other: &Self) -> bool {
        self.matches(other, 1, None, None)
    }
}

impl Eq for ImageMultiHash {}

pub fn average_hash(image: &DynamicImage, hash_size: usize) -> ImageHash {
    let size = cmp::max(2, hash_size);
    let gray = image.to_luma8();
    let resized = imageops::resize(
        &gray,
        size as u32,
        size as u32,
        imageops::FilterType::Lanczos3,
    );
    let pixels: Vec<f64> = resized.pixels().map(|p| p.0[0] as f64).collect();
    let arr = Array2::from_shape_vec((size, size), pixels).unwrap();
    let avg = arr.mean().unwrap_or(0.0);
    let diff = arr.mapv(|x| x > avg);
    ImageHash::from_bool_array(&diff)
}

fn dct_1d(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    let factor = PI / (2.0 * n as f64);
    let mut y = vec![0.0; n];
    for k in 0..n {
        let k_f64 = k as f64;
        let sum: f64 = x
            .iter()
            .enumerate()
            .map(|(i, &xi)| xi * (factor * k_f64 * (2 * i + 1) as f64).cos())
            .sum();
        y[k] = 2.0 * sum;
    }
    y
}

fn dct2(arr: &Array2<f64>) -> Array2<f64> {
    let mut rows_transformed = arr.clone();
    for mut row in rows_transformed.outer_iter_mut() {
        let v: Vec<f64> = row.iter().cloned().collect();
        let dct = dct_1d(&v);
        row.assign(&Array::from_vec(dct));
    }
    let mut cols_transformed = rows_transformed.reversed_axes();
    for mut col in cols_transformed.outer_iter_mut() {
        let v: Vec<f64> = col.iter().cloned().collect();
        let dct = dct_1d(&v);
        col.assign(&Array::from_vec(dct));
    }
    cols_transformed.reversed_axes()
}

pub fn phash(image: &DynamicImage, hash_size: usize, highfreq_factor: usize) -> ImageHash {
    let size = cmp::max(2, hash_size);
    let img_size = size * highfreq_factor;
    let gray = image.to_luma8();
    let resized = imageops::resize(
        &gray,
        img_size as u32,
        img_size as u32,
        imageops::FilterType::Lanczos3,
    );
    let pixels: Vec<f64> = resized.pixels().map(|p| p.0[0] as f64).collect();
    let arr = Array2::from_shape_vec((img_size, img_size), pixels).unwrap();
    let dct = dct2(&arr);
    let dct_low = dct.slice(s![..size, ..size]).to_owned();
    let mut flat: Vec<f64> = dct_low.iter().cloned().collect();
    flat.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = flat.len() / 2;
    let med = if flat.len() % 2 == 0 {
        (flat[mid - 1] + flat[mid]) / 2.0
    } else {
        flat[mid]
    };
    let diff = dct_low.mapv(|x| x > med);
    ImageHash::from_bool_array(&diff)
}

pub fn phash_simple(image: &DynamicImage, hash_size: usize, highfreq_factor: usize) -> ImageHash {
    let size = cmp::max(2, hash_size);
    let img_size = size * highfreq_factor;
    let gray = image.to_luma8();
    let resized = imageops::resize(
        &gray,
        img_size as u32,
        img_size as u32,
        imageops::FilterType::Lanczos3,
    );
    let pixels: Vec<f64> = resized.pixels().map(|p| p.0[0] as f64).collect();
    let arr = Array2::from_shape_vec((img_size, img_size), pixels).unwrap();
    let mut dct = arr.clone();
    for mut row in dct.outer_iter_mut() {
        let v: Vec<f64> = row.iter().cloned().collect();
        row.assign(&Array::from_vec(dct_1d(&v)));
    }
    let end_idx = cmp::min(size + 1, img_size);
    let dct_low = dct.slice(s![..size, 1..end_idx]).to_owned();
    let avg = dct_low.mean().unwrap_or(0.0);
    let diff = dct_low.mapv(|x| x > avg);
    ImageHash::from_bool_array(&diff)
}

pub fn dhash(image: &DynamicImage, hash_size: usize) -> ImageHash {
    let size = cmp::max(2, hash_size);
    let gray = image.to_luma8();
    let resized = imageops::resize(
        &gray,
        (size + 1) as u32,
        size as u32,
        imageops::FilterType::Lanczos3,
    );
    let pixels: Vec<f64> = resized.pixels().map(|p| p.0[0] as f64).collect();
    let arr = Array2::from_shape_vec((size, size + 1), pixels).unwrap();
    let left = arr.slice(s![.., ..size]);
    let right = arr.slice(s![.., 1..]);
    let diff = Zip::from(&right).and(&left).map_collect(|&r, &l| r > l);
    ImageHash::from_bool_array(&diff)
}

pub fn dhash_vertical(image: &DynamicImage, hash_size: usize) -> ImageHash {
    let size = cmp::max(2, hash_size);
    let gray = image.to_luma8();
    let resized = imageops::resize(
        &gray,
        size as u32,
        (size + 1) as u32,
        imageops::FilterType::Lanczos3,
    );
    let pixels: Vec<f64> = resized.pixels().map(|p| p.0[0] as f64).collect();
    let arr = Array2::from_shape_vec((size + 1, size), pixels).unwrap();
    let top = arr.slice(s![..size, ..]);
    let bottom = arr.slice(s![1.., ..]);
    let diff = Zip::from(&bottom).and(&top).map_collect(|&b, &t| b > t);
    ImageHash::from_bool_array(&diff)
}

mod wavelet {
    use ndarray::{Array, Array2};

    const DB4_DEC_LO: [f64; 4] = [
        -0.12940952255092145,
        0.22414386804185735,
        0.836516303737469,
        0.48296291314469025,
    ];
    const DB4_DEC_HI: [f64; 4] = [
        -0.48296291314469025,
        0.836516303737469,
        -0.22414386804185735,
        -0.12940952255092145,
    ];
    const DB4_REC_LO: [f64; 4] = [
        0.48296291314469025,
        0.836516303737469,
        0.22414386804185735,
        -0.12940952255092145,
    ];
    const DB4_REC_HI: [f64; 4] = [
        -0.12940952255092145,
        -0.22414386804185735,
        0.836516303737469,
        -0.48296291314469025,
    ];

    fn convolve_1d_full(data: &[f64], kernel: &[f64]) -> Vec<f64> {
        let n = data.len();
        let k = kernel.len();
        if n == 0 || k == 0 {
            return Vec::new();
        }
        let out_len = n + k - 1;
        let mut out = vec![0.0; out_len];
        for i in 0..n {
            for j in 0..k {
                out[i + j] += data[i] * kernel[j];
            }
        }
        out
    }

    fn upsample_1d(arr: &[f64], factor: usize) -> Vec<f64> {
        let mut out = vec![0.0; arr.len() * factor];
        for (i, &v) in arr.iter().enumerate() {
            out[i * factor] = v;
        }
        out
    }

    pub fn haar_decompose_1d(data: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = data.len();
        let mut approx = Vec::with_capacity(n / 2);
        let mut detail = Vec::with_capacity(n / 2);
        let s = 2.0_f64.sqrt();
        for i in 0..(n / 2) {
            let a = data[2 * i];
            let b = data[2 * i + 1];
            approx.push((a + b) / s);
            detail.push((a - b) / s);
        }
        (approx, detail)
    }

    pub fn haar_reconstruct_1d(approx: &[f64], detail: &[f64]) -> Vec<f64> {
        let n = approx.len() + detail.len();
        let mut out = vec![0.0; n];
        let s = 2.0_f64.sqrt();
        for i in 0..approx.len() {
            let a = approx[i];
            let d = detail[i];
            out[2 * i] = (a + d) / s;
            out[2 * i + 1] = (a - d) / s;
        }
        out
    }

    pub fn db4_decompose_1d(data: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = data.len();
        let conv_lo = convolve_1d_full(data, &DB4_DEC_LO);
        let conv_hi = convolve_1d_full(data, &DB4_DEC_HI);
        let mut approx = Vec::new();
        let mut detail = Vec::new();
        let mut i = 1;
        while i < conv_lo.len() {
            approx.push(conv_lo[i]);
            i += 2;
        }
        let mut j = 1;
        while j < conv_hi.len() {
            detail.push(conv_hi[j]);
            j += 2;
        }
        let approx = approx.into_iter().take(n / 2).collect::<Vec<_>>();
        let detail = detail.into_iter().take(n / 2).collect::<Vec<_>>();
        (approx, detail)
    }

    pub fn db4_reconstruct_1d(approx: &[f64], detail: &[f64]) -> Vec<f64> {
        let n = approx.len() + detail.len();
        let lo_up = upsample_1d(approx, 2);
        let hi_up = upsample_1d(detail, 2);
        let lo_conv = convolve_1d_full(&lo_up, &DB4_REC_LO);
        let hi_conv = convolve_1d_full(&hi_up, &DB4_REC_HI);
        let mut out = vec![0.0; n];
        let start = 3;
        for i in 0..n {
            if start + i < lo_conv.len() && start + i < hi_conv.len() {
                out[i] = lo_conv[start + i] + hi_conv[start + i];
            }
        }
        out
    }

    fn dwt2_decompose<F>(
        arr: &Array2<f64>,
        dec1d: F,
    ) -> (Array2<f64>, Array2<f64>, Array2<f64>, Array2<f64>)
    where
        F: Fn(&[f64]) -> (Vec<f64>, Vec<f64>),
    {
        let (h, w) = arr.dim();
        let mut rows_low = Array2::zeros((h, w / 2));
        let mut rows_high = Array2::zeros((h, w / 2));
        for r in 0..h {
            let row: Vec<f64> = arr.row(r).iter().cloned().collect();
            let (l, hi) = dec1d(&row);
            for c in 0..w / 2 {
                if c < l.len() && c < hi.len() {
                    rows_low[(r, c)] = l[c];
                    rows_high[(r, c)] = hi[c];
                }
            }
        }
        let mut ll = Array2::zeros((h / 2, w / 2));
        let mut lh = Array2::zeros((h / 2, w / 2));
        let mut hl = Array2::zeros((h / 2, w / 2));
        let mut hh = Array2::zeros((h / 2, w / 2));
        for c in 0..w / 2 {
            let col_low: Vec<f64> = rows_low.column(c).iter().cloned().collect();
            let (ll_col, lh_col) = dec1d(&col_low);
            let col_high: Vec<f64> = rows_high.column(c).iter().cloned().collect();
            let (hl_col, hh_col) = dec1d(&col_high);
            for r in 0..h / 2 {
                if r < ll_col.len() && r < lh_col.len() && r < hl_col.len() && r < hh_col.len() {
                    ll[(r, c)] = ll_col[r];
                    lh[(r, c)] = lh_col[r];
                    hl[(r, c)] = hl_col[r];
                    hh[(r, c)] = hh_col[r];
                }
            }
        }
        (ll, lh, hl, hh)
    }

    pub fn wavedec2<F>(
        arr: &Array2<f64>,
        level: usize,
        dec1d: F,
    ) -> (Array2<f64>, Vec<(Array2<f64>, Array2<f64>, Array2<f64>)>)
    where
        F: Fn(&[f64]) -> (Vec<f64>, Vec<f64>),
    {
        let mut current = arr.clone();
        let mut details = Vec::new();
        for _ in 0..level {
            let (h, w) = current.dim();
            if h % 2 != 0 || w % 2 != 0 || h == 0 || w == 0 {
                break;
            }
            let (ll, lh, hl, hh) = dwt2_decompose(&current, &dec1d);
            details.push((lh, hl, hh));
            current = ll;
        }
        details.reverse();
        (current, details)
    }

    fn idwt2_reconstruct<F>(
        ll: &Array2<f64>,
        lh: &Array2<f64>,
        hl: &Array2<f64>,
        hh: &Array2<f64>,
        rec1d: F,
    ) -> Array2<f64>
    where
        F: Fn(&[f64], &[f64]) -> Vec<f64>,
    {
        let h = ll.nrows() * 2;
        let mut rows_low = Array2::zeros((h, ll.ncols()));
        let mut rows_high = Array2::zeros((h, ll.ncols()));
        for c in 0..ll.ncols() {
            let col_ll: Vec<f64> = ll.column(c).iter().cloned().collect();
            let col_lh: Vec<f64> = lh.column(c).iter().cloned().collect();
            let col_hl: Vec<f64> = hl.column(c).iter().cloned().collect();
            let col_hh: Vec<f64> = hh.column(c).iter().cloned().collect();
            let rec_low = rec1d(&col_ll, &col_lh);
            let rec_high = rec1d(&col_hl, &col_hh);
            for r in 0..h {
                if r < rec_low.len() && r < rec_high.len() {
                    rows_low[(r, c)] = rec_low[r];
                    rows_high[(r, c)] = rec_high[r];
                }
            }
        }
        let w = ll.ncols() * 2;
        let mut result = Array2::zeros((h, w));
        for r in 0..h {
            let row_l: Vec<f64> = rows_low.row(r).iter().cloned().collect();
            let row_h: Vec<f64> = rows_high.row(r).iter().cloned().collect();
            let row_full = rec1d(&row_l, &row_h);
            result.row_mut(r).assign(&Array::from_vec(row_full));
        }
        result
    }

    pub fn waverec2<F>(
        coeffs: &(Array2<f64>, Vec<(Array2<f64>, Array2<f64>, Array2<f64>)>),
        rec1d: F,
    ) -> Array2<f64>
    where
        F: Fn(&[f64], &[f64]) -> Vec<f64>,
    {
        let (current, details) = coeffs;
        let mut result = current.clone();
        for (lh, hl, hh) in details.iter() {
            result = idwt2_reconstruct(&result, lh, hl, hh, &rec1d);
        }
        result
    }
}

pub fn whash(
    image: &DynamicImage,
    hash_size: usize,
    image_scale: Option<usize>,
    mode: &str,
    remove_max_haar_ll: bool,
) -> ImageHash {
    let hash_size = if hash_size.is_power_of_two() {
        hash_size
    } else {
        8
    };
    let img_w = image.width() as usize;
    let img_h = image.height() as usize;
    let natural_scale = 2usize.pow((cmp::min(img_w, img_h) as f64).log2() as u32);
    let image_scale = image_scale.unwrap_or_else(|| cmp::max(natural_scale, hash_size));
    let image_scale = if image_scale.is_power_of_two() {
        image_scale
    } else {
        cmp::max(16, hash_size)
    };

    let ll_max_level = (image_scale as f64).log2() as usize;
    let level = (hash_size as f64).log2() as usize;
    let dwt_level = if level <= ll_max_level {
        ll_max_level - level
    } else {
        0
    };

    let gray = image.to_luma8();
    let resized = imageops::resize(
        &gray,
        image_scale as u32,
        image_scale as u32,
        imageops::FilterType::Lanczos3,
    );
    let pixels: Vec<f64> = resized.pixels().map(|p| p.0[0] as f64 / 255.0).collect();
    let mut arr = Array2::from_shape_vec((image_scale, image_scale), pixels).unwrap();

    let (dec1d, _rec1d): (
        fn(&[f64]) -> (Vec<f64>, Vec<f64>),
        fn(&[f64], &[f64]) -> Vec<f64>,
    ) = match mode {
        "db4" => (wavelet::db4_decompose_1d, wavelet::db4_reconstruct_1d),
        _ => (wavelet::haar_decompose_1d, wavelet::haar_reconstruct_1d),
    };

    if remove_max_haar_ll {
        let (ll, details) = wavelet::wavedec2(&arr, ll_max_level, wavelet::haar_decompose_1d);
        let zero_ll = Array2::zeros(ll.dim());
        let mut empty_details = Vec::new();
        for item in details.iter() {
            let dim = item.0.dim();
            empty_details.push((Array2::zeros(dim), Array2::zeros(dim), Array2::zeros(dim)));
        }
        arr = wavelet::waverec2(&(zero_ll, empty_details), wavelet::haar_reconstruct_1d);
    }

    let (ll, _) = wavelet::wavedec2(&arr, dwt_level, dec1d);
    let dwt_low = ll;

    let mut flat: Vec<f64> = dwt_low.iter().cloned().collect();
    flat.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let med = if flat.is_empty() {
        0.0
    } else {
        let mid = flat.len() / 2;
        if flat.len() % 2 == 0 {
            (flat[mid - 1] + flat[mid]) / 2.0
        } else {
            flat[mid]
        }
    };
    let diff = dwt_low.mapv(|x| x > med);
    ImageHash::from_bool_array(&diff)
}

pub fn colorhash(image: &DynamicImage, binbits: usize) -> ImageHash {
    let binbits = cmp::max(1, binbits);
    let rgb_img = image.to_rgb8();
    let (w, h) = rgb_img.dimensions();
    let total = (w * h) as f64;

    let mut intensity = Vec::with_capacity((w * h) as usize);
    let mut hue = Vec::with_capacity((w * h) as usize);
    let mut saturation = Vec::with_capacity((w * h) as usize);

    for y in 0..h {
        for x in 0..w {
            let px = rgb_img.get_pixel(x, y);
            let (r, g, b) = (
                px[0] as f64 / 255.0,
                px[1] as f64 / 255.0,
                px[2] as f64 / 255.0,
            );
            let max_val = r.max(g).max(b);
            let min_val = r.min(g).min(b);
            let delta = max_val - min_val;

            let lum = 0.299 * r + 0.587 * g + 0.114 * b;
            intensity.push((lum * 255.0) as u8);

            let max_val_nonzero = if max_val == 0.0 { 1.0 } else { max_val };
            let s = delta / max_val_nonzero;
            let h_val = if delta == 0.0 {
                0.0
            } else if (max_val - r).abs() < f64::EPSILON {
                60.0 * (((g - b) / delta) % 6.0)
            } else if (max_val - g).abs() < f64::EPSILON {
                60.0 * (((b - r) / delta) + 2.0)
            } else {
                60.0 * (((r - g) / delta) + 4.0)
            };
            let h_val = if h_val < 0.0 { h_val + 360.0 } else { h_val };

            hue.push(((h_val / 360.0) * 255.0).round() as u8);
            saturation.push((s * 255.0).round() as u8);
        }
    }

    let mask_black: Vec<bool> = intensity.iter().map(|&i| i < 32).collect();
    let frac_black = if total > 0.0 {
        mask_black.iter().filter(|&&b| b).count() as f64 / total
    } else {
        0.0
    };

    let mask_gray: Vec<bool> = saturation
        .iter()
        .zip(mask_black.iter())
        .map(|(&s, &b)| !b && s < 85)
        .collect();
    let frac_gray = if total > 0.0 {
        mask_gray.iter().filter(|&&b| b).count() as f64 / total
    } else {
        0.0
    };

    let mask_colors: Vec<bool> = mask_black
        .iter()
        .zip(mask_gray.iter())
        .map(|(&b, &g)| !b && !g)
        .collect();
    let c = mask_colors.iter().filter(|&&b| b).count().max(1) as f64;

    let faint_thresh = 170u8;
    let mask_faint: Vec<bool> = mask_colors
        .iter()
        .zip(saturation.iter())
        .map(|(&col, &s)| col && s < faint_thresh)
        .collect();
    let mask_bright: Vec<bool> = mask_colors
        .iter()
        .zip(saturation.iter())
        .map(|(&col, &s)| col && s >= faint_thresh)
        .collect();

    fn histogram(values: &[u8], mask: &[bool], nbins: usize) -> Vec<usize> {
        let mut counts = vec![0; nbins];
        let bin_width = 256.0 / nbins as f64;
        for (&v, &m) in values.iter().zip(mask.iter()) {
            if m {
                let bin = (v as f64 / bin_width).floor() as usize;
                let bin = bin.min(nbins - 1);
                counts[bin] += 1;
            }
        }
        counts
    }

    let h_faint_counts = histogram(&hue, &mask_faint, 6);
    let h_bright_counts = histogram(&hue, &mask_bright, 6);

    let maxvalue = 2usize.pow(binbits as u32) as f64;
    let mut values: Vec<usize> = Vec::new();
    values.push(cmp::min(
        (maxvalue - 1.0) as usize,
        (frac_black * maxvalue) as usize,
    ));
    values.push(cmp::min(
        (maxvalue - 1.0) as usize,
        (frac_gray * maxvalue) as usize,
    ));
    for cnt in h_faint_counts.iter().chain(h_bright_counts.iter()) {
        let frac = *cnt as f64 / c;
        values.push(cmp::min(
            (maxvalue - 1.0) as usize,
            (frac * maxvalue) as usize,
        ));
    }

    let mut bitarray = Vec::new();
    for &v in &values {
        for i in 0..binbits {
            let mask = 1 << (binbits - i - 1);
            bitarray.push((v & mask) != 0);
        }
    }
    let arr = Array2::from_shape_vec((values.len(), binbits), bitarray)
        .unwrap_or_else(|_| Array2::from_elem((0, 0), false));
    ImageHash::from_bool_array(&arr)
}

fn find_region(
    remaining: &Array2<bool>,
    segmented: &mut HashSet<(usize, usize)>,
) -> HashSet<(usize, usize)> {
    let (h, w) = remaining.dim();
    let mut start = None;
    'outer: for i in 0..h {
        for j in 0..w {
            if remaining[(i, j)] {
                start = Some((i, j));
                break 'outer;
            }
        }
    }
    let start = match start {
        Some(s) => s,
        None => return HashSet::new(),
    };
    let mut in_region = HashSet::new();
    in_region.insert(start);
    let mut new_pixels = in_region.clone();
    let mut not_in_region = HashSet::new();

    loop {
        let mut try_next = HashSet::new();
        for &(x, y) in &new_pixels {
            let neighbours = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ];
            for &n in &neighbours {
                if n.0 < h && n.1 < w && !segmented.contains(&n) && !not_in_region.contains(&n) {
                    try_next.insert(n);
                }
            }
        }
        if try_next.is_empty() {
            break;
        }
        new_pixels.clear();
        for &pix in &try_next {
            if remaining[pix] {
                in_region.insert(pix);
                new_pixels.insert(pix);
                segmented.insert(pix);
            } else {
                not_in_region.insert(pix);
            }
        }
    }
    in_region
}

fn find_all_segments(
    pixels: &Array2<f64>,
    segment_threshold: f64,
    min_segment_size: usize,
) -> Vec<HashSet<(usize, usize)>> {
    let (h, w) = pixels.dim();
    if h == 0 || w == 0 {
        return Vec::new();
    }
    let threshold_pixels = pixels.mapv(|p| p > segment_threshold);
    let mut unassigned = Array2::from_elem((h, w), true);
    let mut already_segmented: HashSet<(usize, usize)> = HashSet::new();
    for i in 0..h {
        already_segmented.insert((i, 0));
        already_segmented.insert((i, w - 1));
    }
    for j in 0..w {
        already_segmented.insert((0, j));
        already_segmented.insert((h - 1, j));
    }

    let mut segments = Vec::new();

    loop {
        let remaining: Array2<bool> = Zip::from(&threshold_pixels)
            .and(&unassigned)
            .map_collect(|&t, &u| t && u);
        if !remaining.iter().any(|&x| x) {
            break;
        }
        let seg = find_region(&remaining, &mut already_segmented);
        if seg.is_empty() {
            break;
        }
        if seg.len() > min_segment_size {
            segments.push(seg.clone());
        }
        for &pix in &seg {
            unassigned[pix] = false;
        }
    }

    let threshold_inv = threshold_pixels.mapv(|p| !p);
    while already_segmented.len() < h * w {
        let remaining: Array2<bool> = Zip::from(&threshold_inv)
            .and(&unassigned)
            .map_collect(|&t, &u| t && u);
        if !remaining.iter().any(|&x| x) {
            break;
        }
        let seg = find_region(&remaining, &mut already_segmented);
        if seg.is_empty() {
            break;
        }
        if seg.len() > min_segment_size {
            segments.push(seg.clone());
        }
        for &pix in &seg {
            unassigned[pix] = false;
        }
    }
    segments
}

fn median_filter_3x3(img: &GrayImage) -> GrayImage {
    let (w, h) = img.dimensions();
    let mut out = img.clone();

    if w < 2 || h < 2 {
        return out;
    }

    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let mut vals = [0u8; 9];
            let mut idx = 0;
            for dy in 0..3 {
                for dx in 0..3 {
                    vals[idx] = img.get_pixel(x + dx - 1, y + dy - 1).0[0];
                    idx += 1;
                }
            }
            vals.sort_unstable();
            out.put_pixel(x, y, Luma([vals[4]]));
        }
    }
    out
}

pub fn crop_resistant_hash<F>(
    image: &DynamicImage,
    hash_func: F,
    limit_segments: Option<usize>,
    segment_threshold: f64,
    min_segment_size: usize,
    segmentation_image_size: u32,
) -> ImageMultiHash
where
    F: Fn(&DynamicImage, usize) -> ImageHash,
{
    let segmentation_image_size = segmentation_image_size.max(4);
    let orig = image.clone();
    let gray = image.to_luma8();
    let seg_img = imageops::resize(
        &gray,
        segmentation_image_size,
        segmentation_image_size,
        imageops::FilterType::Lanczos3,
    );

    let blurred = imageops::blur(&seg_img, 2.0);
    let filtered = median_filter_3x3(&blurred);

    let pixels_vec: Vec<f64> = filtered.pixels().map(|p| p.0[0] as f64).collect();
    let pixels = Array2::from_shape_vec(
        (
            segmentation_image_size as usize,
            segmentation_image_size as usize,
        ),
        pixels_vec,
    )
    .unwrap();

    let mut segments = find_all_segments(&pixels, segment_threshold, min_segment_size);
    if segments.is_empty() {
        let mut whole = HashSet::new();
        whole.insert((0, 0));
        whole.insert((
            segmentation_image_size as usize - 1,
            segmentation_image_size as usize - 1,
        ));
        segments.push(whole);
    }

    if let Some(lim) = limit_segments {
        segments.sort_by(|a, b| b.len().cmp(&a.len()));
        segments.truncate(lim);
    }

    let orig_w = orig.width() as f64;
    let orig_h = orig.height() as f64;
    let scale_w = orig_w / segmentation_image_size as f64;
    let scale_h = orig_h / segmentation_image_size as f64;

    let mut hashes = Vec::new();
    for seg in &segments {
        let min_y = seg.iter().map(|&(y, _)| y).min().unwrap_or(0) as f64;
        let min_x = seg.iter().map(|&(_, x)| x).min().unwrap_or(0) as f64;
        let max_y = seg.iter().map(|&(y, _)| y).max().unwrap_or(0) as f64 + 1.0;
        let max_x = seg.iter().map(|&(_, x)| x).max().unwrap_or(0) as f64 + 1.0;

        let crop_x = (min_x * scale_w) as u32;
        let crop_y = (min_y * scale_h) as u32;
        let crop_w = (((max_x - min_x) * scale_w) as u32)
            .max(1)
            .min(orig.width() - crop_x);
        let crop_h = (((max_y - min_y) * scale_h) as u32)
            .max(1)
            .min(orig.height() - crop_y);

        let cropped = orig.crop_imm(crop_x, crop_y, crop_w, crop_h);
        hashes.push(hash_func(&cropped, 8));
    }
    ImageMultiHash::new(hashes)
}

fn load_raw_rgb(data: *const u8, width: u32, height: u32) -> Option<DynamicImage> {
    if data.is_null() || width == 0 || height == 0 {
        return None;
    }
    let total_bytes = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(3)?;

    let raw_slice = unsafe { std::slice::from_raw_parts(data, total_bytes) };
    let buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_vec(width, height, raw_slice.to_vec())?;
    Some(DynamicImage::ImageRgb8(buffer))
}

#[unsafe(no_mangle)]
pub extern "C" fn imagehash_average_hash(
    data: *const u8,
    width: u32,
    height: u32,
    hash_size: u32,
) -> *mut c_char {
    let img = match load_raw_rgb(data, width, height) {
        Some(i) => i,
        None => return ptr::null_mut(),
    };
    let hash = average_hash(&img, hash_size as usize);
    let hex = hash.to_string();
    match CString::new(hex) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn imagehash_dhash(
    data: *const u8,
    width: u32,
    height: u32,
    hash_size: u32,
) -> *mut c_char {
    let img = match load_raw_rgb(data, width, height) {
        Some(i) => i,
        None => return ptr::null_mut(),
    };
    let hash = dhash(&img, hash_size as usize);
    let hex = hash.to_string();
    match CString::new(hex) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn imagehash_phash(
    data: *const u8,
    width: u32,
    height: u32,
    hash_size: u32,
    highfreq_factor: u32,
) -> *mut c_char {
    let img = match load_raw_rgb(data, width, height) {
        Some(i) => i,
        None => return ptr::null_mut(),
    };
    let hash = phash(&img, hash_size as usize, highfreq_factor as usize);
    let hex = hash.to_string();
    match CString::new(hex) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn imagehash_whash_haar(
    data: *const u8,
    width: u32,
    height: u32,
    hash_size: u32,
    image_scale: i32,
) -> *mut c_char {
    let img = match load_raw_rgb(data, width, height) {
        Some(i) => i,
        None => return ptr::null_mut(),
    };
    let scale = if image_scale < 0 {
        None
    } else {
        Some(image_scale as usize)
    };
    let hash = whash(&img, hash_size as usize, scale, "haar", true);
    let hex = hash.to_string();
    match CString::new(hex) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn imagehash_colorhash(
    data: *const u8,
    width: u32,
    height: u32,
    binbits: u32,
) -> *mut c_char {
    let img = match load_raw_rgb(data, width, height) {
        Some(i) => i,
        None => return ptr::null_mut(),
    };
    let hash = colorhash(&img, binbits as usize);
    let hex = hash.to_string();
    match CString::new(hex) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn imagehash_hamming_distance(hex1: *const c_char, hex2: *const c_char) -> u32 {
    if hex1.is_null() || hex2.is_null() {
        return u32::MAX;
    }
    let s1 = unsafe { std::ffi::CStr::from_ptr(hex1) };
    let s2 = unsafe { std::ffi::CStr::from_ptr(hex2) };
    let str1 = match s1.to_str() {
        Ok(s) => s,
        Err(_) => return u32::MAX,
    };
    let str2 = match s2.to_str() {
        Ok(s) => s,
        Err(_) => return u32::MAX,
    };
    let h1 = hex_to_hash(str1);
    let h2 = hex_to_hash(str2);
    h1.distance(&h2)
}

#[unsafe(no_mangle)]
pub extern "C" fn imagehash_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        };
    }
}