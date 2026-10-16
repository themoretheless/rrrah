//! Common image entrypoint; sensor data and rendered pixels stay distinct.

use crate::{
    DecodeError, DecodeOutput, DecodeRequest, NativeRawDecoder, RasterDecodeError, RawDecoder,
    bounded_io::read_managed,
    sniff::{SniffedFormat, read_header, sniff},
};
use rrrah_core::DecodedRaster;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSourceKind {
    Sensor,
    Raster,
}

#[derive(Debug, Clone)]
pub enum DecodedImage {
    Sensor(Box<DecodeOutput>),
    Raster(DecodedRaster),
}

/// File-extension candidates for the shipped decoders, not a promise that
/// every storage/color variant can be decoded or displayed.
pub fn is_supported_image_path(path: &Path) -> bool {
    is_supported_raw_path(path)
        || path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
            [
                "dcm", "dicom", "ase", "aseprite", "jxr", "wdp", "hdp", "jls", "jng", "jpg", "jpeg", "png",
                "apng", "gif", "webp", "bmp", "ico", "qoi", "tga", "ppm", "pgm", "pbm", "pnm", "pvr", "fits",
                "fit", "fts", "mrc", "mrcs", "map", "nrrd", "wad", "rla", "pic", "cin", "cineon", "dpx",
                "wal", "mac", "macp", "pntg", "mpnt", "pam", "ff", "exr", "hdr", "pfm", "pcx", "sgi", "rgb",
                "rgba", "bw", "ras", "sun", "sunras", "dds", "avif", "xbm", "xpm", "cur", "svg", "ora",
                "pdf", "ai", "eps", "basis", "ktx2", "ktx", "pkm", "iff", "ilbm", "lbm", "pix", "astc",
                "heic", "heif", "hif", "x3f", "pict", "pct", "kra", "xcf", "psd", "psb", "dcx", "mng", "sti",
                "wmf", "emf", "jxl", "jp2", "j2k", "j2c", "jpc", "rpf",
            ]
            .iter()
            .any(|ext| s.eq_ignore_ascii_case(ext))
        })
}

/// Supported RAW extension candidates. A candidate may still contain an
/// unsupported camera/storage variant; content validation happens at decode.
pub fn is_supported_raw_path(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        [
            "eip", "crw", "cr3", "cr2", "dng", "gpr", "nef", "nrw", "arw", "sr2", "mrw", "erf", "kdc", "srw",
            "3fr", "fff", "dcr", "mos", "iiq", "srf", "dcs", "orf", "pef", "ptx", "rw2", "rwl", "raw", "raf",
            "tif", "tiff",
        ]
        .iter()
        .any(|ext| s.eq_ignore_ascii_case(ext))
    })
}

pub fn image_source_kind(request: &DecodeRequest) -> Result<ImageSourceKind, RasterDecodeError> {
    inspect_image_source(request).map(|(kind, _, _)| kind)
}

fn inspect_image_source(request: &DecodeRequest) -> Result<(ImageSourceKind, bool, bool), RasterDecodeError> {
    request.check_cancelled()?;
    // Share one bounded header read between sensor and raster detection.
    let file = std::fs::File::open(&request.path).map_err(|source| DecodeError::Io {
        path: request.path.clone(),
        source,
    })?;
    let (header, length) = read_header(file).map_err(|source| DecodeError::Io {
        path: request.path.clone(),
        source,
    })?;
    let bytes = &header[..length];
    request.check_cancelled()?;
    let eps_candidate = crate::eps::has_magic(bytes);
    Ok((
        classify_image_source(request, bytes)?,
        eps_candidate,
        crate::rpf::has_magic(bytes),
    ))
}

fn classify_image_source(
    request: &DecodeRequest,
    bytes: &[u8],
) -> Result<ImageSourceKind, RasterDecodeError> {
    // EIP opens the original sensor member. Its ZIP assets do not select an
    // ORA/KRA raster or a preview, and Capture One adjustments are not applied.
    if request
        .path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("eip"))
    {
        return Ok(ImageSourceKind::Sensor);
    }
    if bytes.starts_with(b"FOVb") || crate::dicom::has_magic(bytes) || crate::rpf::has_magic(bytes) {
        return Ok(ImageSourceKind::Raster);
    }
    match sniff(&bytes) {
        SniffedFormat::Crw
        | SniffedFormat::Cr3
        | SniffedFormat::Cr2
        | SniffedFormat::Orf
        | SniffedFormat::Rw2
        | SniffedFormat::Mrw
        | SniffedFormat::Raf => Ok(ImageSourceKind::Sensor),
        SniffedFormat::TiffFamily => {
            // Camera extensions remain sensor requests even for malformed or
            // unsupported RAW variants. Never fall back to a TIFF preview.
            let camera = request
                .path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| {
                    [
                        "dng", "nef", "nrw", "arw", "sr2", "mrw", "erf", "kdc", "srw", "3fr", "fff", "dcr",
                        "mos", "iiq", "srf", "dcs", "pef", "ptx", "raw", "mef",
                    ]
                    .iter()
                    .any(|ext| s.eq_ignore_ascii_case(ext))
                });
            if camera
                || crate::dng::has_sensor_image(&read_managed(request)?)
                    .map_err(|error| DecodeError::NativeDng(error.to_string()))?
            {
                Ok(ImageSourceKind::Sensor)
            } else {
                Ok(ImageSourceKind::Raster)
            }
        }
        _ => {
            // An actual raster signature wins over a misleading RAW suffix.
            let raster_magic = crate::eps::has_magic(&bytes)
                || crate::xcf::has_magic(&bytes)
                || crate::pict::has_magic(&bytes)
                || crate::dicom::has_magic(&bytes)
                || crate::aseprite::has_magic(&bytes)
                || crate::jpegxr::has_magic(&bytes)
                || crate::jpegls::has_magic(&bytes)
                || crate::jng::has_magic(&bytes)
                || crate::mng::has_magic(&bytes)
                || crate::sti::has_magic(&bytes)
                || crate::wmf::has_magic(&bytes)
                || crate::emf::has_magic(&bytes)
                || crate::pvr::has_magic(&bytes)
                || crate::fits::has_magic(&bytes)
                || crate::mrc::has_magic(&bytes)
                || crate::nrrd::has_magic(&bytes)
                || crate::wad::has_magic(&bytes)
                || crate::softimage::has_magic(&bytes)
                || crate::cineon::has_magic(&bytes)
                || crate::dpx::has_magic(&bytes)
                || crate::macpaint::has_wrapper(&bytes)
                || crate::astc::has_magic(&bytes)
                || crate::heif::has_magic(&bytes)
                || crate::pdf::has_magic(&bytes)
                || crate::basis::has_magic(&bytes)
                || crate::ktx2::has_magic(&bytes)
                || crate::ktx::has_magic(&bytes)
                || crate::pkm::has_magic(&bytes)
                || crate::iff::has_magic(&bytes)
                || crate::kra::has_magic(&bytes)
                || crate::psd::has_magic(&bytes)
                || crate::dcx::has_magic(&bytes)
                || crate::jp2::has_magic(&bytes)
                || crate::jxl::has_magic(&bytes)
                || crate::ora::has_magic(&bytes)
                || bytes.starts_with(&[0, 0, 2, 0])
                || bytes.starts_with(b"/* XPM */")
                || bytes.starts_with(b"! XPM2")
                || bytes.starts_with(b"DDS ")
                || bytes.starts_with(&[0x59, 0xa6, 0x6a, 0x95])
                || bytes.starts_with(&[1, 218])
                || image::guess_format(&bytes).is_ok()
                || bytes.starts_with(b"PF\n")
                || bytes.starts_with(b"Pf\n")
                || bytes.starts_with(b"PF\r\n")
                || bytes.starts_with(b"Pf\r\n")
                || bytes.first() == Some(&10);
            if !raster_magic && is_supported_raw_path(&request.path) {
                Ok(ImageSourceKind::Sensor)
            } else {
                Ok(ImageSourceKind::Raster)
            }
        }
    }
}

pub fn decode_image_file(path: impl AsRef<Path>) -> Result<DecodedImage, RasterDecodeError> {
    decode_image(&DecodeRequest::new(path.as_ref()))
}

pub fn decode_image(request: &DecodeRequest) -> Result<DecodedImage, RasterDecodeError> {
    let (kind, eps_candidate, rpf_candidate) = inspect_image_source(request)?;
    match kind {
        ImageSourceKind::Sensor => Ok(DecodedImage::Sensor(Box::new(NativeRawDecoder.decode(request)?))),
        ImageSourceKind::Raster => Ok(DecodedImage::Raster(crate::raster::decode_raster_inspected(
            request,
            eps_candidate,
            rpf_candidate,
        )?)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn original_rgb16_psd_psb_display_color_matches_independent_littlecms_double() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for extension in ["psd", "psb"] {
            let name = format!("psd-tools-16bit5x5.{extension}");
            let budget = rrrah_core::MemoryBudget::new(65536);
            let mut request = DecodeRequest::new(root.join(&name)); request.memory_budget = Some(budget.clone());
            let DecodedImage::Raster(native) = decode_image(&request).unwrap() else { panic!() };
            let prepared = crate::prepare_raster_for_display_with_budget(&native, Some(&budget)).unwrap();
            let reference = std::fs::read(root.join(format!("{name}.linear-rgba64le"))).unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(actual) = prepared.pixels() else { panic!() };
            assert_eq!(reference.len(), actual.len() * 8);
            let mut maximum = 0f64;
            for (index, (actual, encoded)) in actual.iter().zip(reference.chunks_exact(8)).enumerate() {
                let expected = f64::from_le_bytes(encoded.try_into().unwrap());
                if index % 4 == 3 { assert_eq!(f64::from(*actual), expected); }
                else { maximum = maximum.max((f64::from(*actual) - expected).abs()); }
            }
            eprintln!("PSD16_LCMS source={name} pixels=25 max_rgb_absolute_error={maximum:.12} declared_bound=0.00002 alpha_exact=true");
            assert!(maximum <= 2e-5, "{name} exceeds declared independent CMS bound: {maximum}");
            drop(native); drop(prepared); assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn pinned_original_rgb16_psd_psb_common_file_route_preserves_samples_and_profile() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for row in include_str!("../../../tests/fixtures/raster/psd-tools-16bit-manifest.tsv").lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields: Vec<_> = row.split('\t').collect();
            let path = root.join(fields[0]);
            let reference = std::fs::read(root.join(fields[3])).unwrap();
            let profile = std::fs::read(root.join(fields[5])).unwrap();
            let source_bytes = std::fs::metadata(&path).unwrap().len();
            let retained = reference.len() as u64 + profile.len() as u64;
            let budget = rrrah_core::MemoryBudget::new(source_bytes + retained);
            let mut request = DecodeRequest::new(path); request.memory_budget = Some(budget.clone());
            let DecodedImage::Raster(image) = decode_image(&request).unwrap() else { panic!() };
            let rrrah_core::RasterPixels::Rgba16(values) = image.pixels() else { panic!() };
            assert!(values.is_managed());
            let actual: Vec<_> = values.iter().flat_map(|value| value.to_le_bytes()).collect();
            assert_eq!(actual, reference);
            assert_eq!(image.color_space(), &rrrah_core::RasterColorSpace::Icc(profile));
            assert_eq!(budget.used(), retained);
            assert_eq!(budget.peak(), source_bytes + retained);
            drop(image); assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn original_gray_alpha_public_file_preserves_oracle_and_exact_source_peak() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let reference = include_bytes!("../../../tests/fixtures/raster/psd-tools-gray0.rgba");
        let profile = include_bytes!("../../../tests/fixtures/raster/psd-tools-gray0.icc");
        let retained = (reference.len()+profile.len()) as u64;
        for ext in ["psd","psb"] {
            let path = root.join(format!("psd-tools-gray0.{ext}"));
            let source = std::fs::metadata(&path).unwrap().len();
            let peak = source+retained+400*359*2;
            let budget = rrrah_core::MemoryBudget::new(peak);
            let mut request = DecodeRequest::new(path);
            let short = rrrah_core::MemoryBudget::new(peak-1);
            request.memory_budget=Some(short.clone());assert!(decode_image(&request).is_err());assert_eq!(short.used(),0);
            request.memory_budget=Some(budget.clone());
            let DecodedImage::Raster(image)=decode_image(&request).unwrap() else {panic!()};
            let rrrah_core::RasterPixels::Rgba8(values)=image.pixels() else {panic!()};
            assert_eq!(&values[..],&reference[..]);assert!(values.is_managed());
            assert_eq!(image.color_space(),&rrrah_core::RasterColorSpace::Icc(profile.to_vec()));
            assert_eq!((budget.used(),budget.peak()),(retained,peak));
            let clone=image.clone();drop(image);assert_eq!(budget.used(),retained);
            drop(clone);assert_eq!(budget.used(),0);
        }
    }

    #[test]
    fn original_high_precision_psd_psb_public_source_peak_and_refusal() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for name in ["4x4_16bit_rgb.psd","4x4_16bit_grayscale.psd","4x4_32bit_rgb.psd","4x4_32bit_grayscale.psd","32bit5x5.psb"] {
            let name = format!("psd-tools-{name}");
            let path = root.join(&name);
            let reference = std::fs::read(root.join(format!("{name}.rgba-le"))).unwrap();
            let profile = std::fs::read(root.join(format!("{name}.icc"))).unwrap();
            let source = std::fs::metadata(&path).unwrap().len();
            let retained = (reference.len()+profile.len()) as u64;
            let mut request = DecodeRequest::new(path);
            let short = rrrah_core::MemoryBudget::new(source+retained-1);
            request.memory_budget = Some(short.clone());
            assert!(decode_image(&request).is_err());
            assert_eq!(short.used(),0);
            let budget = rrrah_core::MemoryBudget::new(source+retained);
            request.memory_budget = Some(budget.clone());
            let DecodedImage::Raster(image) = decode_image(&request).unwrap() else {panic!()};
            let actual = match image.pixels() {
                rrrah_core::RasterPixels::Rgba16(values) => { assert!(values.is_managed());values.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>() },
                rrrah_core::RasterPixels::Rgba32Float(values) => { assert!(values.is_managed());values.iter().flat_map(|v| v.to_bits().to_le_bytes()).collect::<Vec<_>>() },
                _ => panic!("precision changed"),
            };
            assert_eq!(actual,reference);
            assert_eq!(image.color_space(),&rrrah_core::RasterColorSpace::Icc(profile));
            assert_eq!((budget.used(),budget.peak()),(retained,source+retained));
            let clone = image.clone();drop(image);assert_eq!(budget.used(),retained);
            drop(clone);assert_eq!(budget.used(),0);
            eprintln!("ORIGINAL_PSD_PUBLIC name={name} peak={} retained={retained} short_refusal=true final=0",source+retained);
        }
    }

    #[test]
    fn multirow_alpha_psd_psb_public_route_preserves_bits_and_source_decode_peak() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = include_str!("../../../tests/fixtures/raster/psd-tools-alpha-multi-manifest.tsv");
        let mut cases = 0;
        for row in manifest.lines().filter(|line| !line.starts_with('#') && !line.is_empty()) {
            let f: Vec<_> = row.split('\t').collect();
            let width: u64 = f[2].parse().unwrap();
            let height: u64 = f[3].parse().unwrap();
            let depth: u64 = f[4].parse().unwrap();
            let channels: u64 = f[5].parse().unwrap();
            let compression: u8 = f[6].parse().unwrap();
            let path = root.join(f[0]);
            let source = std::fs::metadata(&path).unwrap().len();
            let expected = std::fs::read(root.join(f[1])).unwrap();
            let output = width * height * 4 * (depth / 8);
            let planes = width * height * channels * (depth / 8);
            let decode_peak = match compression {
                0 => output,
                1 => planes + output,
                2 | 3 => planes + rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES.max(output)
                    .max(if compression == 3 && depth == 32 { width * 4 } else { 0 }),
                _ => panic!("fixture compression"),
            };
            let peak = source + decode_peak;
            let mut request = DecodeRequest::new(path);
            request.assume_untagged_linear_srgb = true;
            let short = rrrah_core::MemoryBudget::new(peak - 1);
            request.memory_budget = Some(short.clone());
            assert!(decode_image(&request).is_err(), "{}", f[0]);
            assert_eq!(short.used(), 0);
            let budget = rrrah_core::MemoryBudget::new(peak);
            request.memory_budget = Some(budget.clone());
            let DecodedImage::Raster(image) = decode_image(&request).unwrap() else { panic!("routing changed") };
            let actual: Vec<_> = match image.pixels() {
                rrrah_core::RasterPixels::Rgba16(values) if depth == 16 => {
                    assert!(values.is_managed());
                    values.iter().flat_map(|v| v.to_le_bytes()).collect()
                }
                rrrah_core::RasterPixels::Rgba32Float(values) if depth == 32 => {
                    assert!(values.is_managed());
                    values.iter().flat_map(|v| v.to_bits().to_le_bytes()).collect()
                }
                _ => panic!("native precision changed"),
            };
            assert_eq!(actual, expected, "{}", f[0]);
            assert_eq!((u64::from(image.width()), u64::from(image.height())), (width, height));
            assert_eq!((image.image_index(), image.image_count()), (0, 1));
            assert_eq!(image.color_space(), &rrrah_core::RasterColorSpace::LinearSrgb);
            assert_eq!((budget.used(), budget.peak()), (output, peak));
            let clone = image.clone();
            drop(image);
            assert_eq!(budget.used(), output);
            drop(clone);
            assert_eq!(budget.used(), 0);
            cases += 1;
        }
        assert_eq!(cases, 32);
    }

    #[test]
    fn common_animation_loading_and_display_keep_rational_timing_and_total_plays() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifests=[("gif",include_str!("../../../tests/fixtures/raster/gif-animation-manifest.tsv")),
            ("apng",include_str!("../../../tests/fixtures/raster/apng-manifest.tsv")),
            ("webp",include_str!("../../../tests/fixtures/raster/webp-animation-manifest.tsv"))];
        let mut count=0;
        for (kind,manifest) in manifests {
            for line in manifest.lines().filter(|line| !line.starts_with('#') && !line.is_empty()) {
                let f:Vec<_>=line.split('\t').collect();
                let budget=rrrah_core::MemoryBudget::new(4*1024*1024);
                let mut request=crate::DecodeRequest::new(root.join(f[0]));
                request.image_index=f[1].parse().unwrap();request.memory_budget=Some(budget.clone());
                let raster=crate::decode_raster(&request).unwrap();
                let timing=raster.animation_timing().expect("common route discarded timing");
                let numerator=f[4].parse::<u32>().unwrap();
                let denominator=if kind=="apng" { f[5].parse::<u32>().unwrap() } else { 1 };
                let plays=if kind=="gif" { f[5].parse::<u32>().unwrap()+1 } else { f[if kind=="apng" {6} else {5}].parse::<u32>().unwrap() };
                assert_eq!(u64::from(timing.delay_ms_numerator())*u64::from(denominator),u64::from(numerator)*u64::from(timing.delay_ms_denominator()));
                assert_eq!(timing.num_plays(),plays);
                let ready=crate::prepare_raster_for_display_with_budget(&raster,Some(&budget)).unwrap();
                assert_eq!(ready.animation_timing(),Some(timing));
                drop(ready);drop(raster);assert_eq!(budget.used(),0);count+=1;
            }
        }
        assert!(count>=40);
        eprintln!("ANIMATION_COMMON_TIMING cases={count} final=0");
    }
    #[test]
    fn pvr_eac_srgb_display_admission_cancellation_and_last_owner() {
        use crate::raster_color::RasterColorError;
        use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterError, RasterPixels};
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut cases = 0;
        let mut cancellations = 0;
        for line in include_str!("../../../tests/fixtures/raster/pvr-eac-manifest.tsv")
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields: Vec<_> = line.split('\t').collect();
            let original = std::fs::read(fixtures.join(fields[0])).unwrap();
            let kind = u32::from_le_bytes(original[20..24].try_into().unwrap());
            if !matches!(kind, 0 | 2 | 4 | 6 | 8 | 10 | 13) {
                continue;
            }
            // Remove metadata to isolate the two pixel-plane reservations.
            let metadata = u32::from_le_bytes(original[48..52].try_into().unwrap()) as usize;
            let mut source = original[..52].to_vec();
            source[48..52].fill(0);
            source[16..20].copy_from_slice(&1u32.to_le_bytes());
            source.extend_from_slice(&original[52 + metadata..]);
            let bytes = fields[2].parse::<u64>().unwrap() * fields[3].parse::<u64>().unwrap() * 16;
            let root = MemoryBudget::new(2 * bytes + 64);
            let standing = root.try_reserve(64).unwrap();
            let mut request = crate::DecodeRequest::new("explicit-srgb.pvr");
            request.image_index = fields[1].parse().unwrap();
            request.memory_budget = Some(root.clone());
            let native = crate::pvr::decode(&source, &request).unwrap().raster;
            assert_eq!(native.color_space(), &RasterColorSpace::Srgb);
            assert_eq!(root.used(), bytes + 64);
            let bits = |frame: &DecodedRaster| {
                let RasterPixels::Rgba32Float(p) = frame.pixels() else {
                    panic!()
                };
                p.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
            };
            let original_bits = bits(&native);
            let pressure = root.try_reserve(1).unwrap();
            assert!(matches!(
                crate::prepare_raster_for_display_with_budget(&native, Some(&root)),
                Err(RasterColorError::Frame(RasterError::Memory(_)))
            ));
            assert_eq!(root.used(), bytes + 65);
            assert_eq!(bits(&native), original_bits);
            drop(pressure);
            let calls = std::cell::Cell::new(0usize);
            let prepared =
                crate::prepare_raster_for_display_with_budget_and_cancel(&native, Some(&root), || {
                    calls.set(calls.get() + 1);
                    false
                })
                .unwrap();
            let expected = bits(&prepared);
            assert_eq!(root.used(), 2 * bytes + 64);
            drop(prepared);
            for cutoff in 1..=calls.get() {
                let observed = std::cell::Cell::new(0usize);
                assert!(matches!(
                    crate::prepare_raster_for_display_with_budget_and_cancel(&native, Some(&root), || {
                        observed.set(observed.get() + 1);
                        observed.get() == cutoff
                    }),
                    Err(RasterColorError::Frame(RasterError::Cancelled))
                ));
                assert_eq!(root.used(), bytes + 64);
                assert_eq!(bits(&native), original_bits);
                let retry = crate::prepare_raster_for_display_with_budget(&native, Some(&root)).unwrap();
                assert_eq!(bits(&retry), expected);
                drop(retry);
                cancellations += 1;
            }
            let retry = crate::prepare_raster_for_display_with_budget(&native, Some(&root)).unwrap();
            let retained = retry.clone();
            drop(retry);
            assert!(crate::prepare_raster_for_display_with_budget(&native, Some(&root)).is_err());
            drop(native);
            assert_eq!(root.used(), bytes + 64);
            assert_eq!(bits(&retained), expected);
            let identity = crate::prepare_raster_for_display_with_budget(&retained, Some(&root)).unwrap();
            let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) =
                (retained.pixels(), identity.pixels())
            else {
                panic!()
            };
            assert!(a.ptr_eq(b));
            assert_eq!(root.used(), bytes + 64);
            drop(retained);
            drop(identity);
            assert_eq!(root.used(), 64);
            drop(standing);
            assert_eq!((root.used(), root.peak()), (0, 2 * bytes + 64));
            cases += 1;
        }
        assert_eq!(cases, 224);
        eprintln!("PVR_EAC_SRGB_ADMISSION cases={cases} cancellations={cancellations} final=0");
    }
    #[test]
    fn cancelled_image_entrypoints_precede_header_io_and_memory_admission() {
        for extension in ["eip", "png", "tiff", "cr3", "pdf", "nrrd", "unknown"] {
            let mut request = crate::DecodeRequest::new(format!("/rrrah-absent-cancelled-image.{extension}"));
            request.cancellation = Some(crate::GenerationToken::new(
                std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)),
                1,
            ));
            let budget = rrrah_core::MemoryBudget::new(0);
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                super::image_source_kind(&request),
                Err(crate::RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert!(matches!(
                super::decode_image(&request),
                Err(crate::RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert!(matches!(
                crate::decode_raster(&request),
                Err(crate::RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert!(matches!(
                crate::decode_raster_with_window(&request, crate::ScalarWindow::new(0.0, 1.0).unwrap()),
                Err(crate::RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert_eq!(budget.peak(), 0);
        }
    }
    #[test]
    fn pdf_compatible_ai_candidate_routes_selected_pages_but_refuses_postscript() {
        let path = std::env::temp_dir().join(format!("rrrah-pdf-compatible-{}.AI", std::process::id()));
        assert!(is_supported_image_path(&path));
        // This checks extension routing with our own PDF, not Adobe producer fidelity.
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdf/red-green-pages.pdf");
        std::fs::copy(fixture, &path).unwrap();
        let budget = rrrah_core::MemoryBudget::new(64 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        for index in 0..2 {
            request.image_index = index;
            let image = decode_image(&request).unwrap();
            let DecodedImage::Raster(image) = image else {
                panic!("PDF became sensor data")
            };
            assert_eq!((image.image_index(), image.image_count()), (index, 2));
            let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
                panic!("unexpected PDF pixel representation")
            };
            let expected = if index == 0 {
                [255, 0, 0, 255]
            } else {
                [0, 255, 0, 255]
            };
            assert!(pixels.chunks_exact(4).all(|pixel| pixel == expected));
            drop(image);
            assert_eq!(budget.used(), 0);
        }
        request.image_index = 0;
        std::fs::write(&path, b"%!PS-Adobe-3.0\n%%Creator: Adobe Illustrator\nshowpage\n").unwrap();
        assert!(matches!(
            decode_image(&request),
            Err(RasterDecodeError::UnsupportedFormat)
        ));
        assert_eq!(budget.used(), 0);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    #[ignore = "requires pinned Mamiya ZD source; set RRRAH_MEF_SOURCE"]
    fn unqualified_mef_refuses_public_view_without_preview_fallback_or_leak() {
        let path = std::path::PathBuf::from(std::env::var_os("RRRAH_MEF_SOURCE").expect("MEF source"));
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        assert_eq!(image_source_kind(&request).unwrap(), ImageSourceKind::Sensor);
        let result = decode_image(&request);
        assert!(
            result.is_err(),
            "unqualified camera must not become a preview or guessed-color image"
        );
        let error = result.unwrap_err();
        assert!(error.to_string().contains("native MEF decoder failed"));
        assert!(error.to_string().contains("white balance"));
        eprintln!("MEF public route refusal: {error}");
        assert_eq!(budget.peak(), 0);
        assert_eq!(budget.used(), 0);
    }
    use super::*;

    #[test]
    fn unsupported_mef_tiff_remains_sensor_without_preview_or_full_read() {
        let path = std::env::temp_dir().join(format!("rrrah-mef-routing-{}.MEF", std::process::id()));
        // A rendered TIFF preview cannot establish that the RAW camera's
        // sensor/color route is implemented. MEF stays a sensor request.
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/pattern.profiled.tif");
        std::fs::copy(fixture, &path).unwrap();
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        assert_eq!(image_source_kind(&request).unwrap(), ImageSourceKind::Sensor);
        assert_eq!(budget.peak(), 0);
        assert!(decode_image(&request).is_err());
        assert_eq!(budget.used(), 0);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn camera_candidates_cover_all_native_backends_and_case() {
        for ext in [
            "CR3", "CR2", "DNG", "NEF", "NRW", "nrw", "ARW", "SR2", "sr2", "mrw", "erf", "kdc", "srw", "3fr",
            "fff", "dcr", "mos", "iiq", "srf", "dcs", "ORF", "PEF", "PTX", "ptx", "RW2", "RWL", "RAW", "raw",
            "RAF", "TIFF",
        ] {
            assert!(is_supported_raw_path(Path::new(&format!("image.{ext}"))));
        }
        assert!(!is_supported_raw_path(Path::new("image.jpg")));
    }
    #[test]
    fn camera_tiff_aliases_are_sensor_without_whole_source_admission() {
        for extension in ["NRW", "PTX", "ptx"] {
            let path =
                std::env::temp_dir().join(format!("rrrah-camera-routing-{}.{extension}", std::process::id()));
            std::fs::write(&path, [b'I', b'I', 42, 0, 8, 0, 0, 0]).unwrap();
            let budget = rrrah_core::MemoryBudget::new(0);
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(budget.clone());
            assert!(is_supported_image_path(&path));
            assert_eq!(image_source_kind(&request).unwrap(), ImageSourceKind::Sensor);
            assert_eq!(budget.used(), 0);
            assert!(decode_image(&request).is_err());
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn strong_contents_override_misleading_extensions_in_both_directions() {
        use std::io::Cursor;
        let path = std::env::temp_dir().join(format!("rrrah-router-png-{}.cr3", std::process::id()));
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(1, 1, image::Rgba([11, 22, 33, 44])))
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        std::fs::write(&path, encoded.into_inner()).unwrap();
        assert!(matches!(decode_image_file(&path), Ok(DecodedImage::Raster(_))));
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(0));
        assert_eq!(image_source_kind(&request).unwrap(), ImageSourceKind::Raster);
        assert_eq!(request.memory_budget.as_ref().unwrap().peak(), 0);

        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/jxr-uint8-3.jxr");
        std::fs::copy(fixture, &path).unwrap();
        assert!(matches!(decode_image_file(&path), Ok(DecodedImage::Raster(_))));
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/ase-32-1-2019-1.aseprite");
        std::fs::copy(fixture, &path).unwrap();
        assert!(matches!(decode_image_file(&path), Ok(DecodedImage::Raster(_))));
        std::fs::remove_file(path).unwrap();
        let path = std::env::temp_dir().join(format!("rrrah-router-cr2-{}.jpg", std::process::id()));
        std::fs::write(&path, b"II*\0\x10\0\0\0CR\x02\0\0\0\0\0").unwrap();
        assert!(matches!(
            decode_image_file(&path),
            Err(RasterDecodeError::Source(DecodeError::NativeCamera {
                format: "CR2",
                ..
            }))
        ));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn ordinary_tiff_is_raster_but_sensor_markers_never_fall_back() {
        use std::io::Cursor;
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(1, 1, image::Rgb([11, 22, 33])))
            .write_to(&mut bytes, image::ImageFormat::Tiff)
            .unwrap();
        let path = std::env::temp_dir().join(format!("rrrah-image-router-{}.tif", std::process::id()));
        std::fs::write(&path, bytes.into_inner()).unwrap();
        assert!(matches!(decode_image_file(&path), Ok(DecodedImage::Raster(_))));
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            image_source_kind(&request),
            Err(RasterDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(budget.used(), 0);
        let source_bytes = std::fs::metadata(&path).unwrap().len();
        let budget = rrrah_core::MemoryBudget::new(source_bytes);
        request.memory_budget = Some(budget.clone());
        assert_eq!(image_source_kind(&request).unwrap(), ImageSourceKind::Raster);
        assert_eq!(budget.peak(), source_bytes);
        assert_eq!(budget.used(), 0);

        // TIFF with DNGVersion but no usable image: must fail in RAW decoder.
        let mut bytes = b"II*\0\x08\0\0\0\x01\0".to_vec();
        bytes.extend_from_slice(&50706_u16.to_le_bytes());
        bytes.extend_from_slice(&[1, 0, 4, 0, 0, 0, 1, 4, 0, 0, 0, 0, 0, 0]);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            image_source_kind(&DecodeRequest::new(&path)).unwrap(),
            ImageSourceKind::Sensor
        );
        assert!(matches!(
            decode_image_file(&path),
            Err(RasterDecodeError::Source(DecodeError::NativeDng(_)))
        ));
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod selected_mip_route_regressions {
    use super::*;
    use rrrah_core::{MemoryBudget, RasterPixels};

    struct TemporarySource(std::path::PathBuf);
    impl Drop for TemporarySource {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    fn source(extension: &str) -> TemporarySource {
        use std::io::Write;
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/dds-mips-bc1.dds");
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rrrah-mip-route-{}-{nonce}.{extension}",
            std::process::id()
        ));
        let source = TemporarySource(path);
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&source.0)
            .unwrap();
        output.write_all(&std::fs::read(fixture).unwrap()).unwrap();
        source
    }
    fn selected(request: &DecodeRequest) -> rrrah_core::DecodedRaster {
        assert_eq!(image_source_kind(request).unwrap(), ImageSourceKind::Raster);
        let DecodedImage::Raster(image) = decode_image(request).unwrap() else {
            panic!("texture routed to sensor")
        };
        assert_eq!(
            (
                image.width(),
                image.height(),
                image.image_index(),
                image.image_count()
            ),
            (2, 1, 2, 4)
        );
        let RasterPixels::Rgba8(pixels) = image.pixels() else {
            panic!("wrong texture precision")
        };
        let oracle = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/dds-mips-bc1.dds.mip2.rgba");
        assert_eq!(&pixels[..], std::fs::read(oracle).unwrap());
        image
    }
    #[test]
    fn selected_mip_routes_under_a_sensor_extension() {
        for extension in ["DNG", "NEF", "ARW"] {
            let source = source(extension);
            let mut request = DecodeRequest::new(&source.0);
            request.image_index = 2;
            drop(selected(&request));
            request.image_index = 4;
            assert!(decode_image(&request).is_err());
        }
    }
    #[test]
    fn selected_mip_routes_under_raw_suffix_and_is_bounded() {
        let source = source("CR3");
        let root = MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(&source.0);
        request.image_index = 2;
        request.memory_budget = Some(root.clone());
        let image = selected(&request);
        assert_eq!(root.used(), image.capacity_bytes());
        assert!(root.used() > 0);
        drop(image);
        assert_eq!(root.used(), 0);
        let denied = MemoryBudget::new(0);
        request.memory_budget = Some(denied.clone());
        assert!(decode_image(&request).is_err());
        assert_eq!((denied.used(), denied.peak()), (0, 0));
    }
}

#[cfg(test)]
mod pict_color_route_regressions {
    use super::*;

    #[test]
    fn common_image_route_requires_explicit_color_assumption_for_display() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/pict/short-rgb555.pict");
        let mut request = DecodeRequest::new(path);
        for linear_assumption in [false, true] {
            request.assume_untagged_linear_srgb = linear_assumption;
            let DecodedImage::Raster(image) = decode_image(&request).unwrap() else {
                panic!("PICT routed to sensor")
            };
            assert_eq!(
                image.color_space(),
                &rrrah_core::RasterColorSpace::Unspecified
            );
            assert!(crate::prepare_raster_for_display(&image).is_err());
        }
        request.assume_untagged_srgb = true;
        let DecodedImage::Raster(image) = decode_image(&request).unwrap() else {
            panic!("PICT routed to sensor")
        };
        assert_eq!(
            image.color_space(),
            &rrrah_core::RasterColorSpace::AssumedSrgb
        );
        let display = crate::prepare_raster_for_display(&image).unwrap();
        assert_eq!((display.width(), display.height()), (1, 1));
        let rrrah_core::RasterPixels::Rgba32Float(pixels) = display.pixels() else {
            panic!("display precision")
        };
        assert_eq!(&pixels[..], &[1., 0., 0., 1.]);
    }
}

#[cfg(test)]
mod dicom_big_endian_route_regressions {
    use super::*;
    use std::io::Write;
    #[test]
    fn big_endian_word_pixels_route_by_content_and_preserve_selected_frame() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let name = "dicom-big-endian-odd8-ow-multiframe.dcm";
        let bytes = std::fs::read(fixtures.join(name)).unwrap();
        struct Source(std::path::PathBuf);
        impl Drop for Source {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let path = std::env::temp_dir().join(format!(
            "rrrah-dicom-be-{}-{}.CR3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let source = Source(path);
        file.write_all(&bytes).unwrap();
        drop(file);
        let budget = rrrah_core::MemoryBudget::new(bytes.len() as u64 + 48);
        let mut request = DecodeRequest::new(&source.0);
        request.image_index = 1;
        request.memory_budget = Some(budget.clone());
        let DecodedImage::Raster(image) = decode_image(&request).unwrap() else {
            panic!("Part10 content routed to sensor")
        };
        assert_eq!(
            (
                image.width(),
                image.height(),
                image.image_index(),
                image.image_count()
            ),
            (3, 1, 1, 3)
        );
        assert_eq!(
            image.color_space(),
            &rrrah_core::RasterColorSpace::Unspecified
        );
        let rrrah_core::RasterPixels::Rgba32Float(pixels) = image.pixels() else {
            panic!("scalar precision")
        };
        let expected = std::fs::read(fixtures.join(format!("{name}.1.raw-f32"))).unwrap();
        assert_eq!(
            pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect::<Vec<_>>()
        );
        assert!(crate::prepare_raster_for_display(&image).is_err());
        assert_eq!(budget.used(), 48);
        drop(image);
        assert_eq!(budget.used(), 0);
        let display =
            crate::decode_raster_with_window(&request, crate::ScalarWindow::new(0., 255.).unwrap())
                .unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(pixels) = display.pixels() else {
            panic!("window precision")
        };
        let expected = std::fs::read(fixtures.join(format!("{name}.1.window-f32"))).unwrap();
        assert_eq!(
            pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            display.color_space(),
            &rrrah_core::RasterColorSpace::LinearSrgb
        );
        drop(display);
        assert_eq!(budget.used(), 0);
        request.image_index = 3;
        assert!(decode_image(&request).is_err());
        assert_eq!(budget.used(), 0);
        let denied = rrrah_core::MemoryBudget::new(0);
        request.image_index = 0;
        request.memory_budget = Some(denied.clone());
        assert!(decode_image(&request).is_err());
        assert_eq!((denied.used(), denied.peak()), (0, 0));
    }
}
