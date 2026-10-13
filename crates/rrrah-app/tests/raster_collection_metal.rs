//! Cross-format transport regression, not an independent decoder/color oracle.
#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

use rrrah_cache::{CacheLimits, ImageSwapConfig, RasterRamCache, RasterSwapCache};
use rrrah_core::{DecodedRaster, MemoryBudget, RasterPixels};

fn bits(raster: &DecodedRaster) -> Vec<u32> {
    match raster.pixels() {
        RasterPixels::Rgba8(p) => p.iter().map(|v| u32::from(*v)).collect(),
        RasterPixels::Rgba16(p) => p.iter().map(|v| u32::from(*v)).collect(),
        RasterPixels::Rgba32Float(p) => p.iter().map(|v| v.to_bits()).collect(),
    }
}

#[test]
#[ignore = "cross-format native/prepared swap and frame qualification requires actual Metal"]
fn raster_collection_native_and_prepared_swap_preserve_metal_frames() {
    qualify_collection(None);
}

#[test]
#[ignore = "requires authored 12MP DPX plus .u8 oracle and actual Metal"]
fn large_dpx_native_and_prepared_swap_preserve_every_metal_pixel() {
    let source = std::env::var("RRRAH_LARGE_DPX_SOURCE").expect("RRRAH_LARGE_DPX_SOURCE required");
    qualify_collection(Some(source));
}

fn qualify_collection(external: Option<String>) {
    let gpu = common::qualification_gpu().expect("actual Metal required");
    eprintln!("raster collection adapter: {}", gpu.adapter_name());
    // Independent constant-color oracle catches accidental SDR clipping and
    // padding included in readback. Every pixel of the odd-sized target counts.
    let control = DecodedRaster::new(
        1,
        1,
        RasterPixels::Rgba32Float(std::sync::Arc::new(vec![4., -1., 0.5, 1.]).into()),
        rrrah_core::RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let oracle = gpu.render_raster_half(
        &control,
        rrrah_gpu::ViewParameters {
            viewport: [63., 65.],
            zoom: 4.,
            exposure_stops: 1.,
            ..Default::default()
        },
        [63, 65],
    );
    assert_eq!(oracle.len(), 63 * 65 * 4);
    assert!(
        oracle
            .chunks_exact(4)
            .all(|p| p == [0x4800, 0xc000, 0x3c00, 0x3c00])
    );
    eprintln!("raster collection HDR oracle: 63x65; 8,-2,1,1; every binary16 pixel exact");
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let sources = [
        "raster/pattern.png",
        "raster/cicp-linear-2020.png",
        "raster/cicp-linear-p3.png",
        "raster/cicp-pq-2020.png",
        "raster/cicp-hlg-2020.png",
        "raster/cicp-narrow-709.png",
        "raster/cicp-encoded-p3.png",
        "raster/gamma-gray.png",
        "raster/gamma-gray-packed-1.png",
        "raster/gamma-gray-packed-2.png",
        "raster/gamma-gray-packed-4.png",
        "raster/gamma-chrm-709.png",
        "raster/gamma-chrm-p3.png",
        "raster/pattern.jpg",
        "raster/webp-animation-0.webp",
        "raster/pattern.bmp",
        "raster/pattern.tga",
        "raster/pattern.profiled.tif",
        "raster/gif-animation-disposal-1.gif",
        "raster/pattern.bitmap.ico",
        "qoi/all-opcodes-0.qoi",
        "pcx/indexed.pcx",
        "raster/pattern.ppm",
        "raster/color-alpha15.pam",
        "raster/float-none-rec709-declared.exr",
        "raster/rle-hdr-rec709-declared.hdr",
        "raster/alpha-merged.psd",
        "raster/composite-2-0.psb",
        "raster/pattern.png.ora",
        "raster/pattern.png.kra",
        "xcf/indexed-v1-kind-4.xcf",
        "raster/dds-mips-bc1-srgb.dds",
        "raster/ktx-mips-4ch-8-0.ktx",
        "raster/ktx2-mips-29-0-lu.ktx2",
        "raster/pvr-bgra8.pvr",
        "raster/cropped.pkm",
        "raster/alpha-10x10.astc",
        "raster/hdr-compressed.astc",
        "raster/precision-32-0.ktx2",
        "raster/precision-32-3.ktx2",
        "raster/alpha-container-0.jxl",
        "raster/alpha-container-1.jxl",
        "raster/precision16.jxl",
        "raster/gray-gamma2-alpha16.png",
        "raster/gray-gamma2-16.tif",
        "raster/dpx-8-wide-row-boundary.dpx",
        "raster/avif-alpha-icc.avif",
        "raster/alpha.heic",
        "raster/pattern-rgb.jp2",
        "raster/pattern-rgb.j2k",
        "raster/fits-f32.fits",
        "raster/nrrd-f32-ascii-little.nrrd",
        "raster/mrc-f16-little.mrc",
        "raster/dicom-signed12-rescale.dcm",
        "raster/rla-16-c1-a16-mixed0.rla",
        "sti/etrle-two-images.sti",
        "raster/wad3-textures.wad",
        "raster/ase-16-0-2019-1.aseprite",
        "pdf/cairo-circle.svg",
        "emf/bezier16.emf",
        "wmf/blue.wmf",
        "mng/gray16-trns.mng",
        "raster/apng-compose-0-0.apng",
        "raster/pattern.bitmap.ico.cur",
        "raster/jng-L-8-0.jng",
        "raster/jls-ffmpeg-16-1-0-0.jls",
        "raster/jxr-uint8-3-icc.jxr",
        "raster/adjacent16.ff",
        "basis/etc1s-colored-alpha.basis",
        "raster/pattern.xbm",
        "raster/palette.xpm",
        "raster/two-pages.dcx",
        "raster/pattern.rgb",
        "raster/sun-bgr.ras",
        "raster/ilbm-truecolor.iff",
        "raster/pbm-1-0.iff",
        "pict/drop-pad-rgb.pict",
        "raster/paint-v2-literal.mac",
        "raster/softimage-16-a1-rle0.pic",
        "raster/row-runs-24.pix",
        "raster/transport-linear.pfm",
        "raster/palette-grid.wal",
    ];
    let additional: Vec<_> = [
        include_str!("../../../tests/fixtures/raster/dpx-manifest.tsv"),
        include_str!("../../../tests/fixtures/raster/cineon-manifest.tsv"),
    ].into_iter().flat_map(str::lines)
        .map(|line| format!("raster/{}", line.split('\t').next().unwrap())).collect();
    assert_eq!(additional.len(), 36);
    let sources: Vec<_> = sources.iter().copied().chain(additional.iter().map(String::as_str)).collect();
    let sources = if let Some(source) = external.as_deref() { vec![source] } else { sources };
    let mut selection_count = 0;
    for name in sources.iter().copied() {
        let mut indices = std::collections::VecDeque::from([0]);
        while let Some(image_index) = indices.pop_front() {
            selection_count += 1;
            for prepared_storage in [false, true] {
                let budget = MemoryBudget::new(if external.is_some() { 512 } else { 128 } * 1024 * 1024);
                let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(name));
                request.image_index = image_index;
                request.memory_budget = Some(budget.clone());
                if name.ends_with("cicp-pq-2020.png") { request.pq_reference_white_nits = Some(100.); }
                if name.ends_with("cicp-hlg-2020.png") { request.hlg_display = Some((100., 1000., 1.2)); }
                if name.ends_with("cicp-narrow-709.png") {
                    // Global folder HDR policies must leave this SDR transfer alone.
                    request.pq_reference_white_nits = Some(100.);
                    request.hlg_display = Some((100., 1000., 1.2));
                }
                let linear_import = name.ends_with(".pfm") || name.ends_with("hdr-compressed.astc");
                request.assume_untagged_srgb = !linear_import;
                request.assume_untagged_linear_srgb = linear_import;
                request.rla_alpha_mode = Some(rrrah_decode::RlaAlphaMode::Straight);
                request.rla_color_space = Some(rrrah_decode::RlaColorSpace::Srgb);
                let game = tempfile::tempdir().unwrap();
                if name.ends_with(".wal") {
                    std::fs::create_dir(game.path().join("textures")).unwrap();
                    std::fs::create_dir(game.path().join("pics")).unwrap();
                    std::fs::copy(&request.path, game.path().join("textures/palette-grid.wal")).unwrap();
                    std::fs::copy(
                        fixtures.join("raster/wal-colormap.pcx"),
                        game.path().join("pics/colormap.pcx"),
                    )
                    .unwrap();
                    request.path = game.path().join("textures/palette-grid.wal");
                }
                let rrrah_decode::DecodedImage::Raster(decoded) =
                    rrrah_decode::decode_image(&request).unwrap_or_else(|error| panic!("{name}: {error}"))
                else {
                    panic!("{name}: sensor route")
                };
                assert_eq!(
                    decoded.image_index(),
                    image_index,
                    "{name}: decoder ignored selection"
                );
                if image_index == 0 && !prepared_storage {
                    indices.extend(1..decoded.image_count());
                    let mut invalid = request.clone();
                    invalid.image_index = decoded.image_count();
                    let retained_bytes = budget.used();
                    assert!(rrrah_decode::decode_image(&invalid).is_err(),
                        "{name}: index at image_count must be refused");
                    assert_eq!(budget.used(), retained_bytes,
                        "{name}: out-of-range decode must release its allocations");
                    eprintln!("raster collection bounds: {name}; rejected_index={}; retained_bytes_unchanged", invalid.image_index);
                }
                if name.ends_with(".pfm") {
                    let oracle = std::fs::read(fixtures.join("raster/transport-linear.pfm.rgba32f")).unwrap();
                    let expected: Vec<_> = oracle
                        .chunks_exact(4)
                        .map(|p| u32::from_le_bytes(p.try_into().unwrap()))
                        .collect();
                    assert_eq!(bits(&decoded), expected, "PFM authored top-first binary32 oracle");
                }
                if name.ends_with(".dpx") || name.ends_with(".cin") {
                    let suffix = match decoded.pixels() {
                        RasterPixels::Rgba8(_) => "u8",
                        RasterPixels::Rgba16(_) => "u16",
                        _ => panic!("unexpected DPX/Cineon sample type"),
                    };
                    let oracle = std::fs::read(fixtures.join(format!("{name}.{suffix}"))).unwrap();
                    let expected = if suffix == "u8" {
                        oracle.into_iter().map(u32::from).collect::<Vec<_>>()
                    } else {
                        oracle.chunks_exact(2).map(|v| u32::from(u16::from_le_bytes(v.try_into().unwrap()))).collect()
                    };
                    assert_eq!(bits(&decoded), expected, "DPX/Cineon independent native sample oracle");
                }
                if name.ends_with("alpha-container-0.jxl") || name.ends_with("alpha-container-1.jxl") {
                    let oracle = std::fs::read(fixtures.join(format!("{name}.rgba"))).unwrap();
                    assert_eq!(bits(&decoded), oracle.into_iter().map(u32::from).collect::<Vec<_>>(),
                        "JXL independent libjxl RGBA oracle");
                }
                if name.ends_with("precision16.jxl") {
                    assert!(matches!(decoded.pixels(), RasterPixels::Rgba16(_)));
                    assert_eq!(bits(&decoded), vec![32768,32768,32768,65535,32769,32769,32769,65535],
                        "JXL adjacent authored 16-bit samples");
                }
                if name.ends_with("hdr-compressed.astc") {
                    let oracle = std::fs::read(fixtures.join("raster/hdr-compressed.astc.rgba32f")).unwrap();
                    let expected: Vec<_> = oracle.chunks_exact(4)
                        .map(|p| u32::from_le_bytes(p.try_into().unwrap())).collect();
                    assert_eq!(bits(&decoded), expected, "ASTC reference float oracle");
                }
                if name.ends_with("precision-32-0.ktx2") || name.ends_with("precision-32-3.ktx2") {
                    let expected: Vec<_> = [0.5_f32, 0.1, 0.3, 1.0, 2.0, 0.2, 0.4, 1.0]
                        .into_iter().map(f32::to_bits).collect();
                    assert_eq!(bits(&decoded), expected, "KTX2 authored float samples");
                }
                if name.ends_with(".wal") {
                    let oracle = std::fs::read(fixtures.join("raster/palette-grid.wal.rgba")).unwrap();
                    assert_eq!(
                        bits(&decoded),
                        oracle.into_iter().map(u32::from).collect::<Vec<_>>(),
                        "WAL external PCX palette must match Pillow palette oracle"
                    );
                }
                let mip_oracle = if name.ends_with("dds-mips-bc1-srgb.dds") {
                    Some(format!("{name}.mip{image_index}.rgba"))
                } else if name.ends_with("ktx-mips-4ch-8-0.ktx") || name.ends_with("ktx2-mips-29-0-lu.ktx2") {
                    Some(format!("{name}.mip{image_index}.rgba8"))
                } else {
                    None
                };
                if let Some(oracle) = mip_oracle {
                    let expected = std::fs::read(fixtures.join(oracle)).unwrap();
                    assert_eq!(
                        bits(&decoded),
                        expected.into_iter().map(u32::from).collect::<Vec<_>>(),
                        "{name}: selected mip {image_index} native oracle"
                    );
                }
                // Physical scalar samples need an explicit display window, not an RGB assumption.
                let decoded = if name.ends_with(".fits")
                    || name.ends_with(".nrrd")
                    || name.ends_with(".mrc")
                    || name.ends_with(".dcm")
                {
                    drop(decoded);
                    rrrah_decode::decode_raster_with_window(
                        &request,
                        rrrah_decode::ScalarWindow::new(-100., 1000.).unwrap(),
                    )
                    .unwrap()
                } else {
                    decoded
                };
                let ready =
                    rrrah_decode::prepare_raster_for_display_with_budget(&decoded, Some(&budget)).unwrap();
                if name.contains("gamma-gray-packed-") {
                    let depth = if name.ends_with("-1.png") {1} else if name.ends_with("-2.png") {2} else {4};
                    let maximum = (1u32<<depth)-1;
                    assert_eq!((decoded.width(), decoded.height()), (16,1));
                    assert!(matches!(decoded.color_space(), rrrah_core::RasterColorSpace::Icc(_)));
                    let expected: Vec<u32> = (0..16u32).flat_map(|x| {
                        let code = x & maximum;
                        let value = code*(255/maximum);
                        [value,value,value,if code==1 {0} else {255}]
                    }).collect();
                    assert_eq!(bits(&decoded), expected);
                    let RasterPixels::Rgba32Float(values) = ready.pixels() else {panic!()};
                    for (x,rgba) in values.chunks_exact(4).enumerate() {
                        let code = x as u32 & maximum;
                        let reference = (f64::from(code)/f64::from(maximum)).powi(2);
                        for value in &rgba[..3] {assert!((f64::from(*value)-reference).abs()<1e-6);}
                        assert_eq!(rgba[3],if code==1 {0.} else {1.});
                    }
                } else if name.contains("gamma-chrm-") || name.ends_with("gamma-gray.png") {
                    let p3 = name.ends_with("gamma-chrm-p3.png");
                    assert_eq!((decoded.width(), decoded.height()), (4,1));
                    assert!(matches!(decoded.color_space(), rrrah_core::RasterColorSpace::Icc(_)));
                    let expected: Vec<u32> = [0,32768,32769,65535].into_iter()
                        .flat_map(|code| [code,if p3 {0} else {code},if p3 {0} else {code},32768]).collect();
                    assert_eq!(bits(&decoded), expected);
                    let RasterPixels::Rgba32Float(values) = ready.pixels() else {panic!()};
                    let coefficients = if p3 {[1.224940176,-0.042056955,-0.019637555]} else {[1.;3]};
                    for (i,code) in [0,32768,32769,65535].into_iter().enumerate() {
                        let value = (f64::from(code)/65535.).powi(2);
                        for c in 0..3 {assert!((f64::from(values[i*4+c])-value*coefficients[c]).abs()<0.0001);}
                        assert!((values[i*4+3]-32768./65535.).abs()<1e-7);
                    }
                    assert!(values[4] < values[8]);
                } else if name.contains("cicp-narrow-") {
                    assert_eq!((decoded.width(), decoded.height()), (5, 1));
                    let expected: Vec<u32> = [0,4096,32768,60160,65535].into_iter()
                        .flat_map(|code| [code,code,code,32768]).collect();
                    assert_eq!(bits(&decoded), expected);
                    let RasterPixels::Rgba32Float(values) = ready.pixels() else { panic!() };
                    assert!(values[0] < 0. && values[16] > 1.);
                    assert_eq!(values[4], 0.);
                    assert_eq!(values[12], 1.);
                    for rgba in values.chunks_exact(4) { assert!((rgba[3] - 32768. / 65535.).abs() < 1e-7); }
                } else if name.contains("cicp-") {
                    assert_eq!((decoded.width(), decoded.height()), (3, 1));
                    let RasterPixels::Rgba32Float(values) = ready.pixels() else { panic!() };
                    for rgba in values.chunks_exact(4) { assert!((rgba[3] - 32768. / 65535.).abs() < 1e-7); }
                    if name.contains("cicp-linear-") || name.contains("cicp-encoded-") {
                        assert_eq!(bits(&decoded), vec![0,0,0,32768,65535,0,0,32768,65535,65535,65535,32768]);
                        assert!(matches!(decoded.color_space(), rrrah_core::RasterColorSpace::Cicp { transfer: 8 | 13, .. }));
                        assert!(values[4] > 1. && values[5] < 0., "wide red must retain out-of-sRGB components");
                    }
                    let white = if name.contains("cicp-pq-") {100.} else if name.contains("cicp-hlg-") {10.} else {1.};
                    for channel in 0..3 { assert!((values[8 + channel] - white).abs() < 0.0001); }
                }
                let output = if external.is_some() {
                    assert_eq!((ready.width(), ready.height()), (4000, 3000));
                    [4000, 3000]
                } else { [64, 64] };
                if name.contains("gray-gamma2-") {
                    let RasterPixels::Rgba16(native) = decoded.pixels() else { panic!() };
                    let RasterPixels::Rgba32Float(linear) = ready.pixels() else { panic!() };
                    for (index, value) in [0u16,32768,32769,65535].into_iter().enumerate() {
                        assert_eq!(&native[index*4..index*4+3], &[value;3]);
                        let expected = (f64::from(value) / 65535.).powi(2) as f32;
                        for channel in &linear[index*4..index*4+3] {
                            assert!((*channel - expected).abs() < 0.0001);
                        }
                        assert_eq!(linear[index*4+3], f32::from(native[index*4+3])/65535.);
                    }
                    assert!(linear[4] < linear[8]);
                }
                let view = rrrah_gpu::ViewParameters {
                    viewport: [output[0] as f32, output[1] as f32],
                    zoom: 1.,
                    ..Default::default()
                };
                let before = gpu.render_raster(&ready, view, output);
                let hdr_before = gpu.render_raster_half(&ready, view, output);
                let source = if prepared_storage {
                    drop(decoded);
                    ready
                } else {
                    drop(ready);
                    decoded
                };
                let expected = bits(&source);
                let color = source.color_space().clone();
                let dimensions = (source.width(), source.height());
                let selection = (source.image_index(), source.image_count());
                let geometry = (
                    source.hotspot(),
                    source.pixel_aspect().map(f32::to_bits),
                    source.sample_scale().to_bits(),
                );
                let weight = source.capacity_bytes();
                let directory = tempfile::tempdir().unwrap();
                let swap = RasterSwapCache::<u8>::new_with_budgets(
                    directory.path(),
                    ImageSwapConfig {
                        limits: CacheLimits {
                            max_bytes: if external.is_some() { weight + 4096 } else { 64 * 1024 * 1024 },
                            max_entries: Some(1),
                            ttl: None,
                        },
                        queue_bytes: weight,
                        queue_count: 1,
                        restore_bytes: weight,
                    },
                    MemoryBudget::new(weight),
                    budget.clone(),
                )
                .unwrap();
                let mut ram = RasterRamCache::new(CacheLimits {
                    max_bytes: weight,
                    max_entries: Some(1),
                    ttl: None,
                });
                ram.enable_swap(swap);
                assert!(ram.insert(1, source), "{name}");
                let visible = ram.get_lease(&1).unwrap();
                assert!(
                    !ram.set_limits_and_spill(CacheLimits::bytes(0)),
                    "{name}: visible lease must pin"
                );
                drop(visible);
                assert!(ram.set_limits_and_spill(CacheLimits::bytes(0)));
                let swap = ram.swap().unwrap();
                swap.wait_idle().unwrap();
                assert_eq!(budget.used(), 0, "{name}: spill ownership");
                let mut admitted = false;
                assert!(
                    swap.try_get(&1, || {
                        admitted |= budget.used() > 0;
                        admitted
                    })
                    .unwrap()
                    .is_none()
                );
                assert!(admitted, "{name}: cancellation must follow allocation");
                assert_eq!(budget.used(), 0);
                let pressure = budget.try_reserve(budget.limit()).unwrap();
                assert!(swap.try_get(&1, || false).is_err());
                drop(pressure);
                let restored = swap.try_get(&1, || false).unwrap().unwrap();
                assert_eq!(bits(&restored), expected, "{name}: native/prepared sample bits");
                assert_eq!(restored.color_space(), &color, "{name}: transfer/profile");
                assert_eq!((restored.width(), restored.height()), dimensions);
                assert_eq!((restored.image_index(), restored.image_count()), selection);
                assert_eq!(
                    (
                        restored.hotspot(),
                        restored.pixel_aspect().map(f32::to_bits),
                        restored.sample_scale().to_bits()
                    ),
                    geometry
                );
                let ready =
                    rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
                assert_eq!(
                    gpu.render_raster(&ready, view, output),
                    before,
                    "{name}: frame after restore"
                );
                assert_eq!(
                    gpu.render_raster_half(&ready, view, output),
                    hdr_before,
                    "{name}: linear HDR binary16 frame after restore"
                );
                eprintln!(
                    "raster collection HDR: {name}; index={image_index}; prepared={prepared_storage}; binary16 frame exact"
                );
                drop(ready);
                drop(restored);
                assert_eq!(budget.used(), 0, "{name}: final managed bytes");
                let stats = swap.stats();
                assert_eq!(stats.errors, 0);
                assert_eq!(stats.reads, 1);
                assert_eq!(stats.writes, 1);
                assert_eq!(stats.queued_bytes, 0);
                eprintln!(
                    "raster collection: {name}; index={image_index}; prepared={prepared_storage}; bits/color/frame exact; root=0; queue=0"
                );
            }
        }
    }
    eprintln!(
        "raster collection complete: sources={}; selections={selection_count}; storage_cases={}; root=0; queue=0",
        sources.len(),
        selection_count * 2
    );
}
