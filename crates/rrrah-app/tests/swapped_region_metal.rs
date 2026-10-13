#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
// Shared worker tests use the same explicit GPU fixture as the application units.
use common as navigation_metal_readback;
#[path = "../src/raster_lod_worker.rs"]
mod raster_lod_worker;
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
use rrrah_gpu::{RasterRenderer, ViewParameters};
use rrrah_swap::{SwapLimits, SwapStore};
use std::sync::Arc;

#[test]
fn swapped_packed_regions_render_exactly_after_full_source_release() {
    let gpu = common::qualification_gpu().expect("actual Metal required");
    eprintln!("swapped region adapter: {}", gpu.adapter_name());
    for aspect in [0.5_f32, 1., 2.] {
        for corner in [[0., 0.], [-1., -1.], [1., 1.], [-1., 1.], [1., -1.]] {
            let cpu = MemoryBudget::new(200 * 160 * 16);
            let source = DecodedRaster::new(
                200,
                160,
                RasterPixels::Rgba32Float(
                    Arc::new(
                        (0..200 * 160)
                            .flat_map(|i| {
                                [
                                    (i % 29) as f32 / 8. - 0.5,
                                    (i % 13) as f32 / 4.,
                                    (i / 200) as f32 / 80.,
                                    [0., 0.25, 0.75, 1.][(i % 4) as usize],
                                ]
                            })
                            .collect::<Vec<_>>(),
                    )
                    .into(),
                ),
                RasterColorSpace::LinearSrgb,
            )
            .unwrap()
            .with_pixel_aspect(Some(aspect))
            .unwrap()
            .try_manage_pixels(&cpu)
            .unwrap();
            let scale = (32. / (200. * aspect)).min(32. / 160.) * 8.;
            let pan = [
                corner[0] * (200. * aspect * scale - 64.) / 2.,
                corner[1] * (160. * scale - 64.) / 2.,
            ];
            let parameters = ViewParameters {
                viewport: [64.; 2],
                zoom: 8.,
                pan,
                ..Default::default()
            };
            let sdr = gpu.render_raster(&source, parameters, [64, 64]).pixels;
            let hdr = gpu.render_raster_half(&source, parameters, [64, 64]);
            let rect = RasterRenderer::visible_source_region(&source, parameters)
                .unwrap()
                .unwrap();
            for axis in 0..2 {
                if corner[axis] > 0. {
                    assert_eq!(rect[axis], 0);
                }
                if corner[axis] < 0. {
                    assert_eq!(rect[axis] + rect[axis + 2], [200, 160][axis]);
                }
            }
            let dir = tempfile::tempdir().unwrap();
            let store = SwapStore::new(
                dir.path(),
                SwapLimits {
                    max_bytes: 600000,
                    max_objects: Some(1),
                },
            )
            .unwrap();
            let index = MemoryBudget::new(1024);
            let swapped = rrrah_cache::SwappedRaster::write(&store, &source, &index, || false).unwrap();
            drop(source);
            assert_eq!(cpu.used(), 0);
            let metadata_rect = RasterRenderer::visible_source_region_for_geometry(
                swapped.dimensions(),
                swapped.pixel_aspect(),
                parameters,
            )
            .unwrap()
            .unwrap();
            assert_eq!(metadata_rect, rect);

            let output = MemoryBudget::new(300000);
            let scratch = output.child(65536);
            let mut worker = raster_lod_worker::LodWorker::default();
            let (wake_tx, wake_rx) = crossbeam_channel::bounded(1);
            worker.set_waker(move || {
                let _ = wake_tx.try_send(());
            });
            let generation = Arc::new(std::sync::atomic::AtomicU64::new(1));
            let id = worker
                .submit_region(
                    swapped.clone(),
                    rect,
                    output.clone(),
                    scratch.clone(),
                    1,
                    rrrah_decode::GenerationToken::new(generation, 1),
                )
                .unwrap();
            wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            let completed = worker.try_take().unwrap();
            assert_eq!(completed.id, id);
            let region = rrrah_cache::RasterRegion {
                source_size: swapped.dimensions(),
                rect,
                frame: completed.into_frame().unwrap(),
            };
            drop(worker);
            for linear_hdr in [false, true] {
                let textures = MemoryBudget::new(300000);
                let queued = MemoryBudget::new(300000);
                let mut renderer = if linear_hdr {
                    RasterRenderer::new_linear_hdr_with_budget(&gpu.device, textures.clone())
                } else {
                    RasterRenderer::new_with_budget(
                        &gpu.device,
                        wgpu::TextureFormat::Rgba8UnormSrgb,
                        textures.clone(),
                    )
                }
                .with_upload_queue_budget(queued.clone());
                renderer
                    .upload_packed_region_with_cancel(
                        &gpu.device,
                        &gpu.queue,
                        &region.frame,
                        region.source_size,
                        [rect[0], rect[1]],
                        || false,
                    )
                    .unwrap();
                renderer.update_view(&gpu.queue, parameters);
                if linear_hdr {
                    assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), hdr);
                } else {
                    assert_eq!(gpu.render_existing_raster(&renderer, [64, 64]).pixels, sdr);
                }
                assert!(
                    renderer
                        .upload_packed_region_with_cancel(
                            &gpu.device,
                            &gpu.queue,
                            &region.frame,
                            [1, 1],
                            [0, 0],
                            || false
                        )
                        .is_err()
                );
                let pressure = textures.try_reserve(textures.available_bytes()).unwrap();
                assert!(matches!(
                    renderer.upload_packed_region_with_cancel(
                        &gpu.device,
                        &gpu.queue,
                        &region.frame,
                        region.source_size,
                        [rect[0], rect[1]],
                        || false
                    ),
                    Err(rrrah_gpu::RasterUploadError::Memory(_))
                ));
                drop(pressure);
                assert!(matches!(
                    renderer.upload_packed_region_with_cancel(
                        &gpu.device,
                        &gpu.queue,
                        &region.frame,
                        region.source_size,
                        [rect[0], rect[1]],
                        || true
                    ),
                    Err(rrrah_gpu::RasterUploadError::Cancelled)
                ));
                if linear_hdr {
                    assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), hdr);
                } else {
                    assert_eq!(gpu.render_existing_raster(&renderer, [64, 64]).pixels, sdr);
                }
                drop(renderer);
                assert_eq!((textures.used(), queued.used()), (0, 0));
            }
            drop(region);
            drop(swapped);
            assert_eq!(
                (
                    cpu.used(),
                    output.used(),
                    scratch.used(),
                    index.used(),
                    store.usage().unwrap().bytes
                ),
                (0, 0, 0, 0, 0)
            );
            eprintln!(
                "swapped region exact: aspect={aspect}; corner={corner:?}; rect={rect:?}; SDR/HDR; all credits=0"
            );
        }
    }
}
