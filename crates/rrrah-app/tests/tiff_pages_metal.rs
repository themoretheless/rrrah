#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "actual Metal TIFF selected-page RAM/TTL/swap qualification"]
fn selected_tiff_pages_survive_ram_ttl_swap_and_metal() {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterPixels};
    use rrrah_cache::{CacheLimits, RasterRamCache, RasterSwapCache, ImageSwapConfig};
    use std::{sync::Arc, time::Duration};
    let gpu=common::qualification_gpu().expect("actual GPU required");
    eprintln!("TIFF pages adapter: {}",gpu.adapter_name());
    let view=rrrah_gpu::ViewParameters {viewport:[63.,65.],..Default::default()};
    for name in ["classic-le","classic-be","big-le","big-be"] {
        let path=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/data/tiff-pages-{name}.tif"));
        let root=MemoryBudget::new(4096);let queue=MemoryBudget::new(256);
        let directory=tempfile::tempdir().unwrap();
        let mut ram=RasterRamCache::new(CacheLimits {max_bytes:32,max_entries:Some(1),ttl:Some(Duration::ZERO)});
        ram.enable_swap(RasterSwapCache::new_with_budgets(directory.path(),ImageSwapConfig {
            limits:CacheLimits::bytes(4096),queue_bytes:256,queue_count:1,restore_bytes:256,
        },queue.clone(),root.clone()).unwrap());
        let mut expected=Vec::new();
        for (page,gray) in [(0usize,[11,233]),(1,[197,31])] {
            assert!(ram.set_limits_and_spill(CacheLimits {max_bytes:32,max_entries:Some(1),ttl:None}));
            let mut request=rrrah_decode::DecodeRequest::new(&path);
            request.image_index=page;request.memory_budget=Some(root.clone());request.assume_untagged_srgb=true;request.assume_untagged_linear_srgb=true;
            let frame={
                let decoded=rrrah_decode::decode_raster(&request).unwrap();
                rrrah_decode::prepare_raster_for_display_with_budget(&decoded,Some(&root)).unwrap()
            };
            assert_eq!((frame.image_index(),frame.image_count()),(page,2));
            let values:Vec<u8>=gray.into_iter().flat_map(|v|[v,v,v,255]).collect();
            let reference=DecodedRaster::new(2,1,RasterPixels::Rgba8(Arc::new(values).into()),rrrah_core::RasterColorSpace::Srgb).unwrap().to_linear_srgb().unwrap();
            let sdr=gpu.render_raster(&reference,view,[63,65]).pixels;
            let hdr=gpu.render_raster_half(&reference,view,[63,65]);
            assert_eq!(gpu.render_raster(&frame,view,[63,65]).pixels,sdr);
            assert_eq!(gpu.render_raster_half(&frame,view,[63,65]),hdr);
            assert!(ram.insert_visible(page,frame));
            assert_eq!(ram.spill_expired(),0);
            let lease=ram.get_lease(&page).unwrap();
            let pressure=root.try_reserve(root.available_bytes()).unwrap();
            let hit=ram.get(&page).unwrap();
            assert_eq!(hit.image_index(),page);
            drop(pressure);drop(lease);
            assert!(ram.set_limits_and_spill(CacheLimits {max_bytes:32,max_entries:Some(1),ttl:Some(Duration::ZERO)}));
            assert!(ram.insert_visible(page,hit));
            assert_eq!(ram.spill_expired(),0);
            ram.mark_visible(99);assert_eq!(ram.spill_expired(),1);
            ram.swap().unwrap().wait_idle().unwrap();
            assert_eq!((root.used(),queue.used()),(0,0));
            expected.push((sdr,hdr));
        }
        assert_ne!(expected[0],expected[1]);
        for page in [1usize,0,1] {
            let frame=ram.get(&page).unwrap();
            assert_eq!((frame.image_index(),frame.image_count()),(page,2));
            assert_eq!(gpu.render_raster(&frame,view,[63,65]).pixels,expected[page].0);
            assert_eq!(gpu.render_raster_half(&frame,view,[63,65]),expected[page].1);
            drop(frame);ram.mark_visible(99);ram.spill_expired();ram.swap().unwrap().wait_idle().unwrap();
            assert_eq!((root.used(),queue.used()),(0,0));
        }
        let stats=ram.swap().unwrap().stats();assert_eq!((stats.writes,stats.reads,stats.errors),(2,3,0));
        eprintln!("TIFF pages: {name}; selected metadata; visible TTL pin; full-pressure RAM hit; reverse swap; SDR/HDR exact; final=0");
    }
}

#[test]
#[ignore = "actual Metal independent Deflate TIFF16/float selected-page precision qualification"]
fn independent_compressed_tiff_pages_keep_native_bits_and_metal_swap() {
    use rrrah_core::{MemoryBudget, RasterPixels};
    use rrrah_cache::{CacheLimits, RasterSwapCache, ImageSwapConfig, SpillAdmission};
    let gpu=common::qualification_gpu().expect("actual GPU required");
    eprintln!("TIFF precision adapter: {}",gpu.adapter_name());
    let view=rrrah_gpu::ViewParameters {viewport:[63.,65.],..Default::default()};
    for kind in ["uint16","float32"] { for order in ["little","big"] { for changed in [false,true] {
        let name=format!("precision-{kind}-{order}-{}.tif",if changed {"changed"} else {"same"});
        let path=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../rrrah-dedup/tests/fixtures/tiff").join(&name);
        let root=MemoryBudget::new(4*1024*1024);let queue=MemoryBudget::new(4096);
        let directory=tempfile::tempdir().unwrap();
        let swap:RasterSwapCache<usize>=RasterSwapCache::new_with_budgets(directory.path(),ImageSwapConfig {
            limits:CacheLimits {max_bytes:65536,max_entries:Some(3),ttl:None},
            queue_bytes:4096,queue_count:1,restore_bytes:4096,
        },queue.clone(),root.clone()).unwrap();
        let mut expected=Vec::new();
        for page in 0..3 {
            let mut request=rrrah_decode::DecodeRequest::new(&path);
            request.image_index=page;request.memory_budget=Some(root.clone());
            request.assume_untagged_linear_srgb=true; // Embedded ICC must retain precedence.
            let native=rrrah_decode::decode_raster(&request).unwrap();
            assert_eq!((native.width(),native.height()),(5,4));
            assert_eq!((native.image_index(),native.image_count()),(page,3));
            assert!(matches!(native.color_space(),rrrah_core::RasterColorSpace::Icc(_)));
            for y in 0..4 { for x in 0..5 { for channel in 0..4 {
                let offset=(y*5+x)*4+channel;
                let code=((page*53+y*37+x*19+channel*71)%256) as u16;
                let modified=changed && page==2 && y==2 && x==3 && channel==0;
                match native.pixels() {
                    RasterPixels::Rgba16(values) if kind=="uint16" => {
                        let reference=if channel==3 {65535} else {(code*257)^u16::from(modified)};
                        assert_eq!(values[offset],reference,"{name} page={page} sample={offset}");
                    }
                    RasterPixels::Rgba32Float(values) if kind=="float32" => {
                        let reference=if channel==3 {1.} else {code as f32/255.};
                        let reference=if modified {reference.next_up()} else {reference};
                        assert_eq!(values[offset].to_bits(),reference.to_bits(),"{name} page={page} sample={offset}");
                    }
                    _=>panic!("native TIFF precision lost"),
                }
            }}}
            let frame=rrrah_decode::prepare_raster_for_display_with_budget(&native,Some(&root)).unwrap();
            drop(native);
            assert_eq!((frame.image_index(),frame.image_count()),(page,3));
            let sdr=gpu.render_raster(&frame,view,[63,65]).pixels;
            let hdr=gpu.render_raster_half(&frame,view,[63,65]);
            let RasterPixels::Rgba32Float(values)=frame.pixels() else {panic!()};
            let bits:Vec<_>=values.iter().map(|v|v.to_bits()).collect();
            assert_eq!(swap.try_enqueue(page,frame),SpillAdmission::Queued);
            swap.wait_idle().unwrap();assert_eq!((root.used(),queue.used()),(0,0));
            expected.push((sdr,hdr,bits));
        }
        assert_ne!(expected[0].0,expected[1].0);
        for page in [2usize,0,1,2] {
            let frame=swap.try_get(&page,||false).unwrap().unwrap();
            assert_eq!((frame.image_index(),frame.image_count()),(page,3));
            let RasterPixels::Rgba32Float(values)=frame.pixels() else {panic!()};
            assert_eq!(values.iter().map(|v|v.to_bits()).collect::<Vec<_>>(),expected[page].2);
            assert_eq!(gpu.render_raster(&frame,view,[63,65]).pixels,expected[page].0);
            assert_eq!(gpu.render_raster_half(&frame,view,[63,65]),expected[page].1);
            drop(frame);assert_eq!((root.used(),queue.used()),(0,0));
        }
        assert_eq!((swap.stats().writes,swap.stats().reads,swap.stats().errors),(3,4,0));
        eprintln!("TIFF precision: {name}; native samples exact; ICC selected metadata; reverse swap; SDR/HDR exact; final=0");
    }}}
}

#[test]
#[ignore = "actual Metal authored signed-HDR TIFF float/alpha selection qualification"]
fn signed_hdr_tiff_pages_preserve_alpha_orientation_and_metal() {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterPixels, RasterColorSpace};
    use rrrah_cache::{CacheLimits, RasterSwapCache, ImageSwapConfig, SpillAdmission};
    use std::sync::Arc;
    let gpu=common::qualification_gpu().expect("actual GPU required");
    eprintln!("TIFF signed HDR adapter: {}",gpu.adapter_name());
    let view=rrrah_gpu::ViewParameters {viewport:[63.,65.],..Default::default()};
    for order in ["le","be"] {
        let root=MemoryBudget::new(8192);let queue=MemoryBudget::new(4096);
        let directory=tempfile::tempdir().unwrap();
        let swap:RasterSwapCache<usize>=RasterSwapCache::new_with_budgets(directory.path(),ImageSwapConfig {
            limits:CacheLimits::bytes(65536),queue_bytes:4096,queue_count:1,restore_bytes:4096,
        },queue.clone(),root.clone()).unwrap();
        let path=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/data/tiff-pages-hdr-deflate-{order}.tif"));
        let mut expected=Vec::new();
        for page in 0..3 {
            let mut request=rrrah_decode::DecodeRequest::new(&path);
            request.image_index=page;request.memory_budget=Some(root.clone());
            let unqualified=rrrah_decode::decode_raster(&request).unwrap();
            assert_eq!(unqualified.color_space(),&RasterColorSpace::Unspecified);
            assert!(rrrah_decode::prepare_raster_for_display(&unqualified).is_err());
            drop(unqualified);
            request.assume_untagged_linear_srgb=true;
            let frame=rrrah_decode::decode_raster(&request).unwrap();
            assert_eq!((frame.image_index(),frame.image_count()),(page,3));
            let (width,height)=if page==2 {(2,3)} else {(3,2)};
            assert_eq!((frame.width(),frame.height()),(width,height));
            let mut values=vec![0f32;24];
            for y in 0..2 { for x in 0..3 {
                let pixel=y*3+x;
                let offset=if page==2 {(x*2+1-y)*4} else {pixel*4};
                values[offset..offset+4].copy_from_slice(&[
                    2.+page as f32+x as f32/8., -0.5-y as f32/8.,
                    if pixel%2==0 {-0.0} else {0.25}, [0.25,0.5,1.][x],
                ]);
            }}
            let bits:Vec<_>=values.iter().map(|v|v.to_bits()).collect();
            let RasterPixels::Rgba32Float(actual)=frame.pixels() else {panic!()};
            assert_eq!(actual.iter().map(|v|v.to_bits()).collect::<Vec<_>>(),bits);
            let prepared=rrrah_decode::prepare_raster_for_display_with_budget(&frame,Some(&root)).unwrap();
            drop(frame);
            let frame=prepared;
            let reference=DecodedRaster::new(width,height,RasterPixels::Rgba32Float(Arc::new(values).into()),RasterColorSpace::LinearSrgb).unwrap();
            let sdr=gpu.render_raster(&reference,view,[63,65]).pixels;
            let hdr=gpu.render_raster_half(&reference,view,[63,65]);
            assert!(hdr.iter().any(|v| *v > 0x3c00 && *v < 0x7c00), "HDR target retains above-one components");
            assert!(hdr.iter().any(|v| *v > 0x8000 && *v < 0xfc00), "HDR target retains negative components");
            assert!(hdr.iter().all(|v| v & 0x7c00 != 0x7c00), "HDR target is finite");
            assert_eq!(gpu.render_raster(&frame,view,[63,65]).pixels,sdr);
            assert_eq!(gpu.render_raster_half(&frame,view,[63,65]),hdr);
            assert_eq!(swap.try_enqueue(page,frame),SpillAdmission::Queued);
            swap.wait_idle().unwrap();assert_eq!((root.used(),queue.used()),(0,0));
            expected.push((sdr,hdr,bits));
        }
        for page in [2usize,1,0,2] {
            let pressure=root.try_reserve(root.available_bytes()).unwrap();
            assert!(swap.try_get(&page,||false).is_err());drop(pressure);
            assert!(swap.try_get(&page,||true).unwrap().is_none());assert_eq!(root.used(),0);
            let frame=swap.try_get(&page,||false).unwrap().unwrap();
            assert_eq!((frame.image_index(),frame.image_count()),(page,3));
            let RasterPixels::Rgba32Float(values)=frame.pixels() else {panic!()};
            assert_eq!(values.iter().map(|v|v.to_bits()).collect::<Vec<_>>(),expected[page].2);
            assert_eq!(gpu.render_raster(&frame,view,[63,65]).pixels,expected[page].0);
            assert_eq!(gpu.render_raster_half(&frame,view,[63,65]),expected[page].1);
            drop(frame);assert_eq!((root.used(),queue.used()),(0,0));
        }
        assert_eq!((swap.stats().writes,swap.stats().reads,swap.stats().errors),(3,4,0));
        eprintln!("TIFF signed HDR: {order}; above-one negative signed-zero; straight/associated alpha; orientation6; pressure/cancel retry; native bits and SDR/HDR exact; final=0");
    }
}

#[test]
#[ignore = "actual Metal per-page distinct TIFF ICC preparation/cache qualification"]
fn distinct_tiff_page_profiles_keep_selected_color_through_swap() {
    use rrrah_core::{MemoryBudget, RasterPixels, RasterColorSpace};
    use rrrah_cache::{CacheLimits, RasterSwapCache, ImageSwapConfig, SpillAdmission};
    let gpu=common::qualification_gpu().expect("actual GPU required");
    eprintln!("TIFF page ICC adapter: {}",gpu.adapter_name());
    let path=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/data/tiff-distinct-page-profiles.tif");
    let root=MemoryBudget::new(65536);let queue=MemoryBudget::new(4096);
    let directory=tempfile::tempdir().unwrap();
    let swap:RasterSwapCache<usize>=RasterSwapCache::new_with_budgets(directory.path(),ImageSwapConfig {
        limits:CacheLimits::bytes(65536),queue_bytes:4096,queue_count:1,restore_bytes:4096,
    },queue.clone(),root.clone()).unwrap();
    let view=rrrah_gpu::ViewParameters {viewport:[63.,65.],..Default::default()};
    let oracle=std::fs::read(path.with_extension("rgb8")).unwrap();
    assert_eq!(oracle.len(),18);
    let mut profiles=Vec::new();let mut expected=Vec::new();
    for page in 0..3 {
        let mut request=rrrah_decode::DecodeRequest::new(&path);
        request.image_index=page;request.memory_budget=Some(root.clone());
        // Neither explicit fallback is allowed to override embedded ICC.
        request.assume_untagged_srgb=true;request.assume_untagged_linear_srgb=true;
        let native=rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!((native.image_index(),native.image_count()),(page,3));
        let RasterColorSpace::Icc(profile)=native.color_space() else {panic!()};
        profiles.push(profile.clone());
        let RasterPixels::Rgba8(samples)=native.pixels() else {panic!()};
        assert_eq!(&samples[..], &[128,64,192,255,20,220,100,255]);
        let frame=rrrah_decode::prepare_raster_for_display_with_budget(&native,Some(&root)).unwrap();
        drop(native);
        let RasterPixels::Rgba32Float(values)=frame.pixels() else {panic!()};
        for index in 0..6 {
            let linear=values[(index/3)*4+index%3] as f64;
            let encoded=if linear<=0.0031308 {12.92*linear}
                else {1.055*linear.powf(1./2.4)-0.055};
            let code=(encoded*255.).round().clamp(0.,255.) as u8;
            let reference=oracle[page*6+index];
            assert!(code.abs_diff(reference)<=1,"page={page} component={index}: {code} vs LittleCMS {reference}");
        }
        assert_eq!(values[3],1.);assert_eq!(values[7],1.);
        let bits:Vec<_>=values.iter().map(|v|v.to_bits()).collect();
        let sdr=gpu.render_raster(&frame,view,[63,65]).pixels;
        let hdr=gpu.render_raster_half(&frame,view,[63,65]);
        assert_eq!(swap.try_enqueue(page,frame),SpillAdmission::Queued);
        swap.wait_idle().unwrap();assert_eq!((root.used(),queue.used()),(0,0));
        expected.push((sdr,hdr,bits));
    }
    assert_eq!(profiles[0],profiles[2]);assert_ne!(profiles[0],profiles[1]);
    assert_eq!(expected[0],expected[2]);assert_ne!(expected[0],expected[1]);
    for page in [1usize,2,0,1] {
        let frame=swap.try_get(&page,||false).unwrap().unwrap();
        assert_eq!((frame.image_index(),frame.image_count()),(page,3));
        let RasterPixels::Rgba32Float(values)=frame.pixels() else {panic!()};
        assert_eq!(values.iter().map(|v|v.to_bits()).collect::<Vec<_>>(),expected[page].2);
        assert_eq!(gpu.render_raster(&frame,view,[63,65]).pixels,expected[page].0);
        assert_eq!(gpu.render_raster_half(&frame,view,[63,65]),expected[page].1);
        drop(frame);assert_eq!((root.used(),queue.used()),(0,0));
    }
    assert_eq!((swap.stats().writes,swap.stats().reads,swap.stats().errors),(3,4,0));
    eprintln!("TIFF page ICC: sRGB->linear->sRGB; identical source codes; LittleCMS RGB8 within1; fallback override refused; reverse swap SDR/HDR exact; final=0");
}
