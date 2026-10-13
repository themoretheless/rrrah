//! Native raster region restoration without retaining full source pixels.
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterError, RasterPixels};
use rrrah_swap::{SegmentedSwapHandle, SwapRows, SwapStore};
use std::io::{Cursor, Read};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RasterRegionError {
    #[error(transparent)]
    Swap(#[from] rrrah_swap::SwapError),
    #[error(transparent)]
    Payload(#[from] crate::RasterPayloadError),
    #[error(transparent)]
    Frame(#[from] RasterError),
    #[error("invalid raster region")]
    Invalid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rrrah_core::{RasterColorSpace, RasterPixels};
    use rrrah_swap::SwapLimits;
    use std::sync::Arc;

    #[test]
    fn exact_lod_cache_refusal_corruption_and_truncation_retry() {
        use std::io::{Seek, SeekFrom, Write};
        let directory = tempfile::tempdir().unwrap();
        let store = SwapStore::new(
            directory.path(),
            SwapLimits {
                max_bytes: 100000,
                max_objects: Some(2),
            },
        )
        .unwrap();
        let frame = DecodedRaster::new(
            32,
            32,
            RasterPixels::Rgba32Float(Arc::new([4., -0.5, 2., 0.75].repeat(1024)).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let index = MemoryBudget::new(1024);
        let source = SwappedRaster::write(&store, &frame, &index, || false).unwrap();
        let output = MemoryBudget::new(100000);
        let scratch = MemoryBudget::new(65536);
        drop(
            source
                .downsample_linear([8, 8], &output, &scratch, || false)
                .unwrap(),
        );
        let pressure = output.try_reserve(output.available_bytes()).unwrap();
        assert!(
            source
                .downsample_linear([8, 8], &output, &scratch, || false)
                .is_err()
        );
        assert!(source.cached_lod.lock().unwrap().is_some());
        drop(pressure);
        assert!(matches!(
            source.downsample_linear([8, 8], &output, &scratch, || true),
            Err(RasterRegionError::Swap(rrrah_swap::SwapError::Cancelled))
        ));
        assert!(source.cached_lod.lock().unwrap().is_some());
        for truncate in [false, true] {
            let cache_path = std::fs::read_dir(directory.path())
                .unwrap()
                .flat_map(|entry| std::fs::read_dir(entry.unwrap().path()).unwrap())
                .map(|entry| entry.unwrap().path())
                .find(|path| std::fs::metadata(path).unwrap().len() == 1088)
                .unwrap();
            let mut file = std::fs::OpenOptions::new().write(true).open(cache_path).unwrap();
            if truncate {
                file.set_len(72).unwrap();
            } else {
                file.seek(SeekFrom::Start(100)).unwrap();
                file.write_all(&[255]).unwrap();
            }
            assert!(
                source
                    .downsample_linear([8, 8], &output, &scratch, || false)
                    .is_err()
            );
            assert!(source.cached_lod.lock().unwrap().is_none());
            assert_eq!(
                (output.used(), scratch.used(), store.usage().unwrap().objects),
                (0, 0, 1)
            );
            let rebuilt = source
                .downsample_linear([8, 8], &output, &scratch, || false)
                .unwrap();
            let reference = frame
                .downsample_linear_with_budget_and_cancel([8, 8], &MemoryBudget::new(1024), || false)
                .unwrap();
            let (RasterPixels::Rgba32Float(actual), RasterPixels::Rgba32Float(expected)) =
                (rebuilt.pixels(), reference.pixels())
            else {
                panic!("float expected")
            };
            assert_eq!(&actual[..], &expected[..]);
            drop(rebuilt);
            assert_eq!(store.usage().unwrap().objects, 2);
        }
        drop(source);
        assert_eq!(
            (
                output.used(),
                scratch.used(),
                index.used(),
                store.usage().unwrap().bytes
            ),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn exact_lod_cache_is_optional_under_single_object_quota() {
        let directory = tempfile::tempdir().unwrap();
        let store = SwapStore::new(
            directory.path(),
            SwapLimits {
                max_bytes: 1024,
                max_objects: Some(1),
            },
        )
        .unwrap();
        let frame = DecodedRaster::new(
            2,
            2,
            RasterPixels::Rgba32Float(Arc::new([2., -0.5, 8., 1.].repeat(4)).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let index = MemoryBudget::new(1024);
        let source = SwappedRaster::write(&store, &frame, &index, || false).unwrap();
        let output = MemoryBudget::new(1024);
        let scratch = MemoryBudget::new(65536);
        for _ in 0..2 {
            let lod = source
                .downsample_linear([1, 1], &output, &scratch, || false)
                .unwrap();
            assert_eq!((lod.width(), lod.height()), (1, 1));
            assert_eq!(store.usage().unwrap().objects, 1);
            assert!(source.cached_lod.lock().unwrap().is_none());
            drop(lod);
            assert_eq!((output.used(), scratch.used(), index.used()), (0, 0, 32));
        }
        drop(source);
        assert_eq!((index.used(), store.usage().unwrap().bytes), (0, 0));
    }

    #[test]
    fn streamed_swap_lod_matches_native_area_filter_after_full_source_release() {
        let directory = tempfile::tempdir().unwrap();
        let store = SwapStore::new(
            directory.path(),
            SwapLimits {
                max_bytes: 400000,
                max_objects: Some(2),
            },
        )
        .unwrap();
        let cpu = MemoryBudget::new(129 * 73 * 16);
        let frame = DecodedRaster::new(
            129,
            73,
            RasterPixels::Rgba32Float(
                Arc::new(
                    (0..129 * 73)
                        .flat_map(|i| {
                            [
                                (i % 29) as f32 / 8. - 0.5,
                                (i % 13) as f32 / 4.,
                                (i / 129) as f32 / 40.,
                                [0., 0.25, 0.75, 1.][i % 4],
                            ]
                        })
                        .collect::<Vec<_>>(),
                )
                .into(),
            ),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .with_pixel_aspect(Some(1.25))
        .unwrap()
        .with_sample_scale(1.5)
        .unwrap()
        .with_hotspot(Some((127, 72)))
        .unwrap()
        .with_image_selection(2, 4)
        .unwrap()
        .try_manage_pixels(&cpu)
        .unwrap();
        let sizes = [[1, 1], [3, 2], [17, 11], [64, 37], [129, 73]];
        let references: Vec<_> = sizes
            .iter()
            .map(|size| {
                frame
                    .downsample_linear_with_budget_and_cancel(*size, &MemoryBudget::new(200000), || false)
                    .unwrap()
            })
            .collect();
        let index = MemoryBudget::new(1024);
        let source = SwappedRaster::write(&store, &frame, &index, || false).unwrap();
        drop(frame);
        assert_eq!(cpu.used(), 0);
        for (limit, admitted) in [(71135, false), (71136, true)] {
            let root = MemoryBudget::new(limit);
            let scratch = root.child(65536);
            assert_eq!(source.can_downsample_linear([17, 11], &root, &scratch), admitted);
            assert_eq!((root.used(), root.peak()), (0, 0));
            if admitted {
                drop(
                    source
                        .downsample_linear([17, 11], &root, &scratch, || false)
                        .unwrap(),
                );
                assert_eq!(root.used(), 0);
                assert_eq!(root.peak(), 71136);
            }
        }
        let output = MemoryBudget::new(300000);
        let scratch = output.child(65536);
        let pressure = output.try_reserve(output.available_bytes()).unwrap();
        assert!(
            source
                .downsample_linear([17, 11], &output, &scratch, || false)
                .is_err()
        );
        drop(pressure);
        assert_eq!(output.used(), 0);
        for (size, reference) in sizes.into_iter().zip(references) {
            let result = source
                .downsample_linear(size, &output, &scratch, || false)
                .unwrap();
            assert_eq!(result.pixel_aspect(), reference.pixel_aspect());
            assert_eq!(result.sample_scale(), reference.sample_scale());
            assert_eq!(result.hotspot(), reference.hotspot());
            assert_eq!((result.image_index(), result.image_count()), (2, 4));
            let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) =
                (result.pixels(), reference.pixels())
            else {
                panic!()
            };
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b.iter()) {
                assert_eq!(a.to_bits(), b.to_bits(), "size={size:?}");
            }
            assert_eq!(output.used(), u64::from(size[0]) * u64::from(size[1]) * 16);
            drop(result);
            assert_eq!(output.used(), 0);
        }
        let mut checks = 0;
        assert!(matches!(
            source.downsample_linear([17, 11], &output, &scratch, || {
                checks += 1;
                checks > 10
            }),
            Err(RasterRegionError::Swap(rrrah_swap::SwapError::Cancelled))
        ));
        assert_eq!(output.used(), 0);
        assert!(
            source
                .downsample_linear([0, 1], &output, &scratch, || false)
                .is_err()
        );
        assert!(
            source
                .downsample_linear([130, 1], &output, &scratch, || false)
                .is_err()
        );
        drop(source);
        assert_eq!(
            (
                output.used(),
                scratch.used(),
                index.used(),
                store.usage().unwrap().bytes
            ),
            (0, 0, 0, 0)
        );
        eprintln!(
            "streamed swapped LOD:5 native area-filter references exact with odd edges alpha signed HDR and metadata; full source released; final credits=0"
        );
    }

    #[test]
    fn region_admission_checks_parse_and_shared_scratch_phases_without_reservation() {
        let directory = tempfile::tempdir().unwrap();
        let store = SwapStore::new(
            directory.path(),
            SwapLimits {
                max_bytes: 100000,
                max_objects: Some(1),
            },
        )
        .unwrap();
        let frame = DecodedRaster::new(
            64,
            64,
            RasterPixels::Rgba32Float(Arc::new([8., -0.5, 0.25, 1.].repeat(64 * 64)).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let index = MemoryBudget::new(1024);
        let source = SwappedRaster::write(&store, &frame, &index, || false).unwrap();
        let rect = [2, 3, 8, 8];
        let scratch = MemoryBudget::new(65536);
        assert!(
            !source
                .can_restore_region(rect, &MemoryBudget::new(2111), &scratch)
                .unwrap()
        );
        let output = MemoryBudget::new(2112);
        assert!(source.can_restore_region(rect, &output, &scratch).unwrap());
        assert_eq!(
            (output.used(), output.peak(), scratch.used(), scratch.peak()),
            (0, 0, 0, 0)
        );
        let crop = source.restore_region(rect, &output, &scratch, || false).unwrap();
        drop(crop);
        for (limit, expected) in [(66623, false), (66624, true)] {
            let root = MemoryBudget::new(limit);
            let output = root.child(3000);
            let scratch = root.child(65536);
            assert_eq!(
                source.can_restore_region(rect, &output, &scratch).unwrap(),
                expected
            );
            assert_eq!((root.used(), root.peak()), (0, 0));
            if expected {
                drop(source.restore_region(rect, &output, &scratch, || false).unwrap());
            }
            assert_eq!(root.used(), 0);
        }
        assert!(
            source
                .can_restore_region([64, 0, 1, 1], &output, &scratch)
                .is_err()
        );
        drop(source);
        assert_eq!((index.used(), store.usage().unwrap().bytes), (0, 0));
    }

    #[test]
    #[ignore = "manual 36MP float source above full-payload512MiB cap"]
    fn large_source_above_full_restore_cap_streams_and_restores_small_native_crop() {
        fn sample(x: usize, y: usize, channel: usize) -> f32 {
            match channel {
                0 => 8. + (x % 17) as f32 / 16.,
                1 => -0.5 - (y % 19) as f32 / 32.,
                2 => {
                    if (x + y) % 2 == 0 {
                        -0.
                    } else {
                        0.25
                    }
                }
                _ => 1.,
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let store = SwapStore::new(
            directory.path(),
            SwapLimits {
                max_bytes: 600_000_000,
                max_objects: Some(1),
            },
        )
        .unwrap();
        let cpu = MemoryBudget::new(6000 * 6000 * 16);
        let frame = DecodedRaster::new(
            6000,
            6000,
            RasterPixels::Rgba32Float(
                Arc::new(
                    (0..6000 * 6000 * 4usize)
                        .map(|i| sample((i / 4) % 6000, (i / 4) / 6000, i % 4))
                        .collect::<Vec<_>>(),
                )
                .into(),
            ),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .with_pixel_aspect(Some(2.))
        .unwrap()
        .try_manage_pixels(&cpu)
        .unwrap();
        assert!(
            crate::raster_payload_len(&frame).is_err(),
            "full restore cap remains unchanged"
        );
        let reference = frame
            .downsample_linear_with_budget_and_cancel([17, 11], &MemoryBudget::new(2992), || false)
            .unwrap();
        let index = MemoryBudget::new(300000);
        let swapped = SwappedRaster::write(&store, &frame, &index, || false).unwrap();
        drop(frame);
        assert_eq!(cpu.used(), 0);
        assert_eq!(store.usage().unwrap().bytes, 576_000_064);
        let output = MemoryBudget::new(100000);
        let scratch = output.child(65536);
        let region = swapped
            .restore_region([5983, 5989, 17, 11], &output, &scratch, || false)
            .unwrap();
        assert_eq!(region.source_size, [6000, 6000]);
        assert_eq!(region.frame.pixel_aspect(), Some(2.));
        let RasterPixels::Rgba32Float(pixels) = region.frame.pixels() else {
            panic!()
        };
        for (i, value) in pixels.iter().enumerate() {
            assert_eq!(
                value.to_bits(),
                sample(5983 + (i / 4) % 17, 5989 + (i / 4) / 17, i % 4).to_bits()
            );
        }
        assert_eq!(output.used(), 17 * 11 * 16);
        eprintln!(
            "36MP segmented source:576000000 source bytes released; crop2992 bytes; index={} scratch_peak={}",
            index.used(),
            scratch.peak()
        );
        drop(region);
        let lod_budget = MemoryBudget::new(300000);
        let lod_scratch = lod_budget.child(65536);
        let lod = swapped
            .downsample_linear([17, 11], &lod_budget, &lod_scratch, || false)
            .unwrap();
        let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) = (lod.pixels(), reference.pixels())
        else {
            panic!()
        };
        for (a, b) in a.iter().zip(b.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        assert_eq!(lod.pixel_aspect(), reference.pixel_aspect());
        assert_eq!(lod_budget.used(), 2992);
        eprintln!(
            "36MP streamed native LOD:17x11 exact direct-source reference; managed peak={} scratch_peak={}; full source remains released",
            lod_budget.peak(),
            lod_scratch.peak()
        );
        drop(lod);
        drop(reference);
        assert_eq!((lod_budget.used(), lod_scratch.used()), (0, 0));
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
    }

    #[test]
    fn hdr_region_survives_release_of_full_source_and_preserves_metadata_and_bits() {
        for color in [
            RasterColorSpace::Cicp {
                primaries: 9,
                transfer: 16,
                matrix: 0,
                full_range: true,
            },
            RasterColorSpace::LinearRgb(
                rrrah_core::RgbChromaticities::new([
                    0.713, 0.293, 0.165, 0.830, 0.128, 0.044, 0.32168, 0.33767,
                ])
                .unwrap(),
            ),
            RasterColorSpace::LinearRgb(
                rrrah_core::RgbChromaticities::new([
                    0.713, 0.293, 0.165, 0.830, 0.128, 0.044, 0.32168, 0.33767,
                ])
                .unwrap()
                .with_adopted_neutral([0.34567, 0.35850])
                .unwrap(),
            ),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let store = SwapStore::new(
                directory.path(),
                SwapLimits {
                    max_bytes: 3_000_000,
                    max_objects: Some(1),
                },
            )
            .unwrap();
            let source_budget = MemoryBudget::new(512 * 320 * 16);
            let sample = |i: usize| [8_f32, -0_f32, -2_f32, f32::from_bits(0x7fc01234)][i % 4];
            let frame = DecodedRaster::new(
                512,
                320,
                RasterPixels::Rgba32Float(
                    Arc::new((0..512 * 320 * 4).map(sample).collect::<Vec<_>>()).into(),
                ),
                color.clone(),
            )
            .unwrap()
            .with_sample_scale(1.5)
            .unwrap()
            .with_pixel_aspect(Some(1.25))
            .unwrap()
            .with_hotspot(Some((300, 160)))
            .unwrap()
            .with_image_selection(2, 3)
            .unwrap()
            .try_manage_pixels(&source_budget)
            .unwrap();
            let index = MemoryBudget::new(4096);
            let swapped = SwappedRaster::write(&store, &frame, &index, || false).unwrap();
            assert_eq!(swapped.pixel_aspect(), Some(1.25));
            drop(frame);
            assert_eq!(
                source_budget.used(),
                0,
                "swapped raster must not retain original pixels"
            );
            let output = MemoryBudget::new(100000);
            let scratch = output.child(65536);
            let rect = [292, 153, 17, 11];
            let region = swapped.restore_region(rect, &output, &scratch, || false).unwrap();
            assert_eq!((region.source_size, region.rect), ([512, 320], rect));
            assert_eq!((region.frame.width(), region.frame.height()), (17, 11));
            assert_eq!(region.frame.hotspot(), Some((8, 7)));
            assert_eq!(region.frame.pixel_aspect(), Some(1.25));
            assert_eq!(region.frame.sample_scale(), 1.5);
            assert_eq!((region.frame.image_index(), region.frame.image_count()), (2, 3));
            assert_eq!(region.frame.color_space(), &color);
            let RasterPixels::Rgba32Float(pixels) = region.frame.pixels() else {
                panic!()
            };
            for (i, value) in pixels.iter().enumerate() {
                assert_eq!(value.to_bits(), sample(i).to_bits());
            }
            assert_eq!(output.used(), 17 * 11 * 16);
            drop(region);
            assert_eq!((output.used(), scratch.used()), (0, 0));
            for rect in [[0, 0, 0, 1], [512, 0, 1, 1], [0, 320, 1, 1], [u32::MAX, 0, 1, 1]] {
                assert!(matches!(
                    swapped.restore_region(rect, &output, &scratch, || false),
                    Err(RasterRegionError::Invalid)
                ));
            }
            let small = MemoryBudget::new(1);
            assert!(
                swapped
                    .restore_region([0, 0, 1, 1], &small, &scratch, || false)
                    .is_err()
            );
            assert_eq!(small.used(), 0);
            assert!(matches!(
                swapped.restore_region([0, 0, 1, 1], &output, &scratch, || true),
                Err(RasterRegionError::Swap(rrrah_swap::SwapError::Cancelled))
            ));
            let outside = swapped
                .restore_region([0, 0, 2, 2], &output, &scratch, || false)
                .unwrap();
            assert_eq!(outside.frame.hotspot(), None);
            drop(outside);
            drop(swapped);
            assert_eq!(
                (
                    source_budget.used(),
                    index.used(),
                    output.used(),
                    store.usage().unwrap().bytes
                ),
                (0, 0, 0, 0)
            );
            eprintln!(
                "swapped HDR region: full2621440-byte source released;17x11 native crop; CICP/aspect/selection/scale/hotspot and all sample bits exact; credits=0"
            );
        }
    }

    #[test]
    fn integer_regions_preserve_icc_samples_and_retry_after_pressure() {
        let directory = tempfile::tempdir().unwrap();
        for wide in [false, true] {
            let store = SwapStore::new(
                directory.path(),
                SwapLimits {
                    max_bytes: 100000,
                    max_objects: Some(1),
                },
            )
            .unwrap();
            let pixels = if wide {
                RasterPixels::Rgba16(
                    Arc::new((0..80 * 40 * 4).map(|i| (i * 7919) as u16).collect::<Vec<_>>()).into(),
                )
            } else {
                RasterPixels::Rgba8(
                    Arc::new((0..80 * 40 * 4).map(|i| (i * 73) as u8).collect::<Vec<_>>()).into(),
                )
            };
            let frame = DecodedRaster::new(80, 40, pixels, RasterColorSpace::Icc(vec![9; 257])).unwrap();
            let index = MemoryBudget::new(1024);
            let swapped = SwappedRaster::write(&store, &frame, &index, || false).unwrap();
            assert_eq!(swapped.pixel_aspect(), None);
            drop(frame);
            let output = MemoryBudget::new(100000);
            let scratch = output.child(65536);
            let pressure = output.try_reserve(output.available_bytes()).unwrap();
            assert!(
                swapped
                    .restore_region([3, 7, 9, 5], &output, &scratch, || false)
                    .is_err()
            );
            drop(pressure);
            let region = swapped
                .restore_region([3, 7, 9, 5], &output, &scratch, || false)
                .unwrap();
            assert_eq!(region.frame.color_space(), &RasterColorSpace::Icc(vec![9; 257]));
            for y in 0..5 {
                for x in 0..9 {
                    for channel in 0..4 {
                        let source = ((y + 7) * 80 + x + 3) * 4 + channel;
                        let target = (y * 9 + x) * 4 + channel;
                        match region.frame.pixels() {
                            RasterPixels::Rgba8(p) => assert_eq!(p[target], (source * 73) as u8),
                            RasterPixels::Rgba16(p) => assert_eq!(p[target], (source * 7919) as u16),
                            _ => panic!(),
                        }
                    }
                }
            }
            drop(region);
            drop(swapped);
            assert_eq!(
                (
                    index.used(),
                    output.used(),
                    scratch.used(),
                    store.usage().unwrap().objects
                ),
                (0, 0, 0, 0)
            );
        }
    }
}

/// A cropped frame plus its location in the original image's coordinate system.
#[derive(Debug, Clone)]
pub struct RasterRegion {
    pub source_size: [u32; 2],
    pub rect: [u32; 4],
    pub frame: DecodedRaster,
}

/// Managed source index and swap object, plus at most one exact LOD swap object.
/// The larger cached level survives temporary pressure downgrades. Both objects
/// and their indices share the caller's disk/index limits and source lifetime;
/// no original CPU pixels or unaccounted LOD pixel allocations are retained.
/// Cache admission failure is optional and does not fail successful filtering.
#[derive(Debug, Clone)]
pub struct SwappedRaster {
    store: SwapStore,
    handle: SegmentedSwapHandle,
    header: [u8; 64],
    size: [u32; 2],
    prefix: u64,
    bytes_per_pixel: u64,
    index_budget: MemoryBudget,
    cached_lod: std::sync::Arc<std::sync::Mutex<Option<([u32; 2], SegmentedSwapHandle)>>>,
}
impl SwappedRaster {
    /// Persist a validated native raster via the existing versioned payload codec.
    /// Blocking I/O belongs on a worker. Source size is bounded by disk quota;
    /// each restored crop retains the ordinary payload decoder size limit.
    pub fn write(
        store: &SwapStore,
        frame: &DecodedRaster,
        index_budget: &MemoryBudget,
        cancelled: impl FnMut() -> bool,
    ) -> Result<Self, RasterRegionError> {
        let (header, profile, bytes) = crate::raster_payload::region_source_descriptor(frame)?;
        let prefix = 64 + profile.len() as u64;
        let bytes_per_pixel = match header[16] {
            1 => 4,
            2 => 8,
            3 => 16,
            _ => return Err(RasterRegionError::Invalid),
        };
        let handle = store.write_segmented_stream(bytes, index_budget, cancelled, |mut writer| {
            crate::raster_payload::write_region_source_payload(&mut writer, frame)
                .map_err(std::io::Error::other)
        })?;
        Ok(Self {
            store: store.clone(),
            handle,
            header,
            size: [frame.width(), frame.height()],
            prefix,
            bytes_per_pixel,
            index_budget: index_budget.clone(),
            cached_lod: Default::default(),
        })
    }
    pub fn dimensions(&self) -> [u32; 2] {
        self.size
    }
    /// Physical pixel aspect retained without loading source pixels or ICC data.
    pub fn pixel_aspect(&self) -> Option<f32> {
        let value = f32::from_le_bytes(self.header[60..64].try_into().unwrap());
        ((self.header[..8] == *b"RRRAST2\0"
            || self.header[..8] == *b"RRRAST3\0"
            || self.header[..8] == *b"RRRAST4\0")
            && value != 0.0)
            .then_some(value)
    }

    /// Area-filter verified source rows directly from swap. Managed storage is
    /// output pixels, one f64 accumulator row, one source byte row and swap scratch.
    /// RGB is accumulated in premultiplied linear space with f64 weights.
    pub fn downsample_linear(
        &self,
        target: [u32; 2],
        budget: &MemoryBudget,
        scratch: &MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<DecodedRaster, RasterRegionError> {
        if cancelled() {
            return Err(rrrah_swap::SwapError::Cancelled.into());
        }
        if target.contains(&0) || (0..2).any(|a| target[a] > self.size[a]) {
            return Err(RasterRegionError::Invalid);
        }
        if self.header[16] != 3 || self.header[17] != 2 {
            return Err(RasterError::ColorTransformRequired.into());
        }
        let cached = self.cached_lod.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some((size, handle)) = cached.filter(|(size, _)| *size == target) {
            struct Input<'a, F> {
                reader: rrrah_swap::SegmentedSwapReader,
                offset: u64,
                length: u64,
                cancelled: &'a mut F,
            }
            impl<F: FnMut() -> bool> Read for Input<'_, F> {
                fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                    let count = bytes.len().min((self.length - self.offset) as usize);
                    self.reader
                        .read_into(self.offset, &mut bytes[..count], &mut *self.cancelled)
                        .map_err(std::io::Error::other)?;
                    self.offset += count as u64;
                    Ok(count)
                }
            }
            let reader = match self.store.segmented_reader(&handle, scratch, &mut cancelled) {
                Ok(reader) => reader,
                Err(error) => {
                    if !matches!(
                        error,
                        rrrah_swap::SwapError::Memory(_) | rrrah_swap::SwapError::Cancelled
                    ) {
                        let mut cache = self.cached_lod.lock().unwrap_or_else(|e| e.into_inner());
                        if cache.as_ref().is_some_and(|(key, _)| *key == size) {
                            *cache = None;
                        }
                    }
                    return Err(error.into());
                }
            };
            let mut input = Input {
                reader,
                offset: 0,
                length: handle.byte_len(),
                cancelled: &mut cancelled,
            };
            let result =
                crate::raster_payload::read_raster_payload_with_length(&mut input, handle.byte_len(), budget);
            if cancelled() {
                return Err(rrrah_swap::SwapError::Cancelled.into());
            }
            let frame = match result {
                Ok(frame) => frame,
                Err(error) => {
                    let mut cache = self.cached_lod.lock().unwrap_or_else(|e| e.into_inner());
                    if !matches!(error, crate::RasterPayloadError::Memory(_))
                        && cache.as_ref().is_some_and(|(key, _)| *key == size)
                    {
                        *cache = None;
                    }
                    return Err(error.into());
                }
            };
            if cancelled() {
                return Err(rrrah_swap::SwapError::Cancelled.into());
            }
            return Ok(frame);
        }
        // Verify metadata before allocating/filtering output, without restoring pixels.
        drop(
            self.store
                .restore_range(&self.handle, 0, self.prefix, budget, scratch, &mut cancelled)?,
        );
        let count = (target[0] as usize)
            .checked_mul(target[1] as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(RasterRegionError::Invalid)?;
        let mut output = budget
            .try_buffer(count, 0_f32)
            .map_err(crate::RasterPayloadError::Memory)?;
        let mut sums = budget
            .try_buffer(target[0] as usize, [0_f64; 4])
            .map_err(crate::RasterPayloadError::Memory)?;
        let scale = [
            f64::from(self.size[0]) / f64::from(target[0]),
            f64::from(self.size[1]) / f64::from(target[1]),
        ];
        let stride = u64::from(self.size[0]) * 16;
        let mut reader = self
            .store
            .segmented_reader(&self.handle, scratch, &mut cancelled)?;
        let mut row = budget
            .try_buffer(stride as usize, 0_u8)
            .map_err(crate::RasterPayloadError::Memory)?;
        let mut loaded_row = None;
        for oy in 0..target[1] {
            sums.fill([0.; 4]);
            let y0 = f64::from(oy) * scale[1];
            let y1 = (f64::from(oy + 1) * scale[1]).min(f64::from(self.size[1]));
            for y in y0.floor() as u32..(y1.ceil() as u32).min(self.size[1]) {
                if cancelled() {
                    return Err(rrrah_swap::SwapError::Cancelled.into());
                }
                if loaded_row != Some(y) {
                    reader.read_into(self.prefix + u64::from(y) * stride, &mut row, &mut cancelled)?;
                    loaded_row = Some(y);
                }
                let wy = (y1.min(f64::from(y + 1)) - y0.max(f64::from(y))).max(0.);
                for ox in 0..target[0] {
                    let x0 = f64::from(ox) * scale[0];
                    let x1 = (f64::from(ox + 1) * scale[0]).min(f64::from(self.size[0]));
                    let sum = &mut sums[ox as usize];
                    for x in x0.floor() as u32..(x1.ceil() as u32).min(self.size[0]) {
                        if x.is_multiple_of(4096) && cancelled() {
                            return Err(rrrah_swap::SwapError::Cancelled.into());
                        }
                        let begin = x as usize * 16;
                        let rgba: [f32; 4] = std::array::from_fn(|c| {
                            f32::from_le_bytes(row[begin + c * 4..begin + c * 4 + 4].try_into().unwrap())
                        });
                        if rgba.iter().any(|v| !v.is_finite()) {
                            return Err(RasterError::NonFiniteSample.into());
                        }
                        if !(0.0..=1.0).contains(&rgba[3]) {
                            return Err(RasterError::InvalidAlpha.into());
                        }
                        let wx = (x1.min(f64::from(x + 1)) - x0.max(f64::from(x))).max(0.);
                        let alpha = f64::from(rgba[3]) * wx * wy;
                        for c in 0..3 {
                            sum[c] += f64::from(rgba[c]) * alpha;
                        }
                        sum[3] += alpha;
                    }
                }
            }
            for ox in 0..target[0] {
                let x0 = f64::from(ox) * scale[0];
                let x1 = (f64::from(ox + 1) * scale[0]).min(f64::from(self.size[0]));
                let sum = sums[ox as usize];
                let begin = (oy as usize * target[0] as usize + ox as usize) * 4;
                if sum[3] > 0. {
                    for c in 0..3 {
                        output[begin + c] = (sum[c] / sum[3]) as f32;
                    }
                }
                output[begin + 3] = (sum[3] / ((x1 - x0) * (y1 - y0))) as f32;
            }
        }
        if cancelled() {
            return Err(rrrah_swap::SwapError::Cancelled.into());
        }
        let hotspot = (self.header[18] == 1).then(|| {
            let coordinate = |offset| u32::from_le_bytes(self.header[offset..offset + 4].try_into().unwrap());
            (
                (u64::from(coordinate(24)) * u64::from(target[0]) / u64::from(self.size[0])) as u32,
                (u64::from(coordinate(28)) * u64::from(target[1]) / u64::from(self.size[1])) as u32,
            )
        });
        let selection = |offset| {
            usize::try_from(u64::from_le_bytes(
                self.header[offset..offset + 8].try_into().unwrap(),
            ))
            .map_err(|_| RasterRegionError::Invalid)
        };
        let frame = DecodedRaster::new(
            target[0],
            target[1],
            RasterPixels::Rgba32Float(output.freeze().into()),
            RasterColorSpace::LinearSrgb,
        )?
        .with_pixel_aspect(Some(
            (f64::from(self.pixel_aspect().unwrap_or(1.)) * scale[0] / scale[1]) as f32,
        ))?
        .with_sample_scale(f32::from_le_bytes(self.header[20..24].try_into().unwrap()))?
        .with_hotspot(hotspot)?
        .with_image_selection(selection(32)?, selection(40)?)?;
        // This optional exact level shares source disk/index quotas. Retain at
        // most one level, preserving the larger exact level across pressure
        // downgrades so recovery can reuse it. Release a smaller predecessor
        // before attempting its replacement.
        // A full quota must never turn a successfully filtered frame into failure.
        let keep_larger = self
            .cached_lod
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|(size, _)| (0..2).all(|a| size[a] >= target[a]));
        if !keep_larger {
            *self.cached_lod.lock().unwrap_or_else(|e| e.into_inner()) = None;
            if let Ok(bytes) = crate::raster_payload::raster_payload_len(&frame) {
                if let Ok(handle) = self.store.write_segmented_stream(
                    bytes,
                    &self.index_budget,
                    &mut cancelled,
                    |mut writer| {
                        crate::raster_payload::write_raster_payload(&mut writer, &frame)
                            .map_err(std::io::Error::other)
                    },
                ) {
                    if !cancelled() {
                        *self.cached_lod.lock().unwrap_or_else(|e| e.into_inner()) = Some((target, handle));
                    }
                }
            }
        }
        if cancelled() {
            return Err(rrrah_swap::SwapError::Cancelled.into());
        }
        Ok(frame)
    }

    /// Advisory admission for streamed linear LOD, including shared scratch credit.
    pub fn can_downsample_linear(
        &self,
        target: [u32; 2],
        budget: &MemoryBudget,
        scratch: &MemoryBudget,
    ) -> bool {
        if target.contains(&0) || (0..2).any(|a| target[a] > self.size[a]) {
            return false;
        }
        if let Some((_, handle)) = self
            .cached_lod
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .filter(|(size, _)| *size == target)
        {
            return u64::from(target[0])
                .checked_mul(u64::from(target[1]))
                .and_then(|n| n.checked_mul(16))
                .is_some_and(|bytes| budget.can_reserve_with(bytes, scratch, handle.byte_len().min(65536)));
        }
        let workspace = u64::from(target[0])
            .checked_mul(u64::from(target[1]))
            .and_then(|n| n.checked_mul(16))
            .and_then(|n| n.checked_add(u64::from(target[0]) * 32))
            .and_then(|n| n.checked_add(u64::from(self.size[0]) * 16));
        let block = self.handle.byte_len().min(65536);
        budget.can_reserve_with(self.prefix, scratch, block)
            && workspace.is_some_and(|bytes| budget.can_reserve_with(bytes, scratch, block))
    }

    /// Advisory admission for both restore phases, including shared ancestors.
    /// Allocation remains authoritative if concurrent ownership changes afterward.
    pub fn can_restore_region(
        &self,
        rect: [u32; 4],
        output: &MemoryBudget,
        scratch: &MemoryBudget,
    ) -> Result<bool, RasterRegionError> {
        let [x, y, width, height] = rect;
        if width == 0
            || height == 0
            || u64::from(x) + u64::from(width) > u64::from(self.size[0])
            || u64::from(y) + u64::from(height) > u64::from(self.size[1])
        {
            return Err(RasterRegionError::Invalid);
        }
        let packed = u64::from(width) * u64::from(height) * self.bytes_per_pixel;
        if packed > crate::raster_payload::MAX_PIXELS {
            return Ok(false);
        }
        let profile = self.prefix - 64;
        let retained = self
            .prefix
            .checked_add(packed)
            .ok_or(RasterRegionError::Invalid)?;
        let parsed = retained
            .checked_add(packed)
            .and_then(|v| v.checked_add(profile))
            .ok_or(RasterRegionError::Invalid)?;
        Ok(
            output.can_reserve_with(retained, scratch, self.handle.byte_len().min(65536))
                && parsed <= output.available_bytes(),
        )
    }

    /// Restore only selected pixel rows; retain source geometry in the returned
    /// descriptor. Sample bits, color declarations, scale, aspect and image selection
    /// survive; hotspot is translated into the crop if contained, otherwise absent.
    /// Managed transient RAM includes packed bytes plus decoded crop/profile and one
    /// swap block. Full source pixel allocation is never restored here.
    pub fn restore_region(
        &self,
        rect: [u32; 4],
        budget: &MemoryBudget,
        scratch: &MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<RasterRegion, RasterRegionError> {
        let [x, y, width, height] = rect;
        if width == 0
            || height == 0
            || u64::from(x) + u64::from(width) > u64::from(self.size[0])
            || u64::from(y) + u64::from(height) > u64::from(self.size[1])
        {
            return Err(RasterRegionError::Invalid);
        }
        let prefix =
            self.store
                .restore_range(&self.handle, 0, self.prefix, budget, scratch, &mut cancelled)?;
        let stride = u64::from(self.size[0]) * self.bytes_per_pixel;
        let pixels = self.store.restore_rows(
            &self.handle,
            SwapRows {
                offset: self.prefix + u64::from(y) * stride + u64::from(x) * self.bytes_per_pixel,
                row_bytes: u64::from(width) * self.bytes_per_pixel,
                stride,
                rows: u64::from(height),
            },
            budget,
            scratch,
            &mut cancelled,
        )?;
        let mut header = self.header;
        header[8..12].copy_from_slice(&width.to_le_bytes());
        header[12..16].copy_from_slice(&height.to_le_bytes());
        if header[18] == 1 {
            let hx = u32::from_le_bytes(header[24..28].try_into().unwrap());
            let hy = u32::from_le_bytes(header[28..32].try_into().unwrap());
            if hx >= x && hy >= y && hx - x < width && hy - y < height {
                header[24..28].copy_from_slice(&(hx - x).to_le_bytes());
                header[28..32].copy_from_slice(&(hy - y).to_le_bytes());
            } else {
                header[18] = 0;
                header[24..32].fill(0);
            }
        }
        if cancelled() {
            return Err(rrrah_swap::SwapError::Cancelled.into());
        }
        let mut reader = Cursor::new(header)
            .chain(Cursor::new(&prefix[64..]))
            .chain(Cursor::new(&*pixels));
        let frame =
            crate::read_raster_payload_with_length(&mut reader, self.prefix + pixels.len() as u64, budget)?;
        if cancelled() {
            return Err(rrrah_swap::SwapError::Cancelled.into());
        }
        Ok(RasterRegion {
            source_size: self.size,
            rect,
            frame,
        })
    }
}
