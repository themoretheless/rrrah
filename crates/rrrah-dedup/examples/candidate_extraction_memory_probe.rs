use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    gradient::{GradientCellRecipe, GradientMatchPolicy},
    local::{LocalPolicy, MatchPolicy},
    local_scan::{
        LocalFilePolicy, ProjectiveGradientPyramidFilePolicy, ProjectivePyramidPhotometricFilePolicy,
    },
    warp::{
        ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy, WarpPolicy,
    },
};
fn policy() -> ProjectiveGradientPyramidFilePolicy {
    let warp = WarpPolicy {
        tolerance: 0.03,
        max_source_pixels: 409600,
    };
    ProjectiveGradientPyramidFilePolicy {
        search: ProjectivePyramidPhotometricFilePolicy {
            local: LocalFilePolicy {
                decode: AnimationBudget {
                    max_frames: 1,
                    max_pixels: 200000,
                    max_file_bytes: 1048576,
                },
                extract: LocalPolicy {
                    max_pixels: 200000,
                    max_candidates: 200000,
                    max_features: 500,
                    minimum_corner_score: 0.0001,
                },
                matching: MatchPolicy {
                    max_comparisons: 4000000,
                    max_distance: 64,
                },
                geometry: GeometryPolicy {
                    tolerance: 2.,
                    min_inliers: 10,
                    max_points: 2000,
                    max_hypotheses: 4096,
                },
                pixels: warp,
                minimum_compared_pixels: 1000,
                minimum_coverage_fraction: 0.3,
                minimum_matched_fraction: 0.9,
            },
            max_levels: 4,
            max_total_pixels: 400000,
            max_total_features: 2000,
            sampling: ProjectiveSamplingPolicy {
                trials: 4096,
                seed: 0x1234abcd,
            },
            photometric: PhotometricPolicy {
                residual: warp,
                minimum_samples: 16,
                minimum_variance: 1e-5,
                minimum_gain: 0.2,
                maximum_gain: 5.,
                maximum_offset: 0.1,
            },
            filter: ColorFilterPolicy {
                filter: FilterPolicy {
                    radius: 3,
                    max_sample_pairs: 32000000,
                },
                color_space: FilterColorSpace::LinearSrgb,
            },
            fit_mode: PhotometricFitMode::ConstrainedLeastSquares,
        },
        matching: GradientMatchPolicy {
            max_comparisons: 4000000,
            max_squared_distance: 0.5,
            squared_ratio: 0.64,
        },
        max_total_gradient_samples: 1024000,
    }
}
fn original_regions_config(
    blur7: bool,
    grid8: bool,
) -> rrrah_dedup::local_scan::ProjectiveDistinctScaleRegionsFilePolicy {
    let mut p = policy();
    p.search.local.decode.max_pixels = 6400000;
    p.search.local.decode.max_file_bytes = 5242880;
    p.search.local.extract.max_pixels = 6400000;
    p.search.local.extract.max_candidates = 6400000;
    p.search.local.extract.max_features = 2000;
    p.search.local.pixels.max_source_pixels = 8000000;
    p.search.local.geometry.max_points = 14000;
    p.search.max_levels = 7;
    p.search.max_total_features = 14000;
    p.search.max_total_pixels = 12800000;
    p.search.photometric.residual.max_source_pixels = 8000000;
    p.search.filter.filter.max_sample_pairs = 1200000000;
    if blur7 {
        p.search.filter.filter.radius = 7;
        p.search.filter.filter.max_sample_pairs = 16000000000;
    }
    p.max_total_gradient_samples = 7168000;
    p.matching.max_comparisons = 392000000;
    rrrah_dedup::local_scan::ProjectiveDistinctScaleRegionsFilePolicy {
        gradient: p,
        recipe: GradientCellRecipe::Interpolated,
        spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
            columns: 4,
            rows: 4,
            max_per_cell: 128,
        },
        competitor_radius: 2.,
        grid: if grid8 { (8, 8) } else { (4, 4) },
        max_regions: if grid8 { 128 } else { 32 },
        max_total_sample_pairs: (if grid8 { 130 } else { 34 }) * p.search.filter.filter.max_sample_pairs,
    }
}

fn main() {
    let args:Vec<_>=std::env::args().skip(1).collect(); assert!(args.len()==2 || args.len()==3);
    let retained_features:usize=args[1].parse().unwrap();
    let policy=original_regions_config(true,true);let g=policy.gradient;
    let ceiling:u64=args.get(2).map(|v|v.parse().unwrap()).unwrap_or(512*1024*1024);
    let budget=rrrah_core::MemoryBudget::new(ceiling);
    println!("memory ceiling={ceiling}");
    let held=budget.try_reserve((retained_features*std::mem::size_of::<rrrah_dedup::gradient::GradientFeature>()) as u64).unwrap();
    println!("prior feature lower bound bytes={}",held.bytes());
    let request=rrrah_decode::DecodeRequest::new(&args[0]);
    let image=rrrah_dedup::decode::decode_selected_frame_bounded(&request,g.search.local.decode,&budget,||false).unwrap();
    println!("decoded used={} peak={}",budget.used(),budget.peak());
    let view=image.view(||false).unwrap();
    let factors:Vec<_>=(0..=6).map(|n|2f64.powf(f64::from(n)/2.)).collect();
    let mut batches=Vec::new();
    for radius in [None,Some(2),Some(4),Some(0)] {
        let samples=radius.map(|r|rrrah_dedup::gradient_scale::smooth_gradient_candidates_managed(&view,r,128000000,&budget,||false));
        let samples=match samples {Some(Ok(v))=>Some(v),Some(Err(e))=>{println!("smooth {radius:?}: {e:?} used={} peak={}",budget.used(),budget.peak());break},None=>None};
        let (w,h)=view.dimensions();
        let smooth=samples.as_ref().map(|v|rrrah_dedup::linear::LinearRgbaView::new(w,h,v,g.search.local.decode.max_pixels,||false).unwrap());
        let mut local=g.search.local.extract;if radius.is_none(){local.minimum_corner_score=0.000001;}
        println!("extract {radius:?} begin used={} peak={}",budget.used(),budget.peak());
        let result=rrrah_dedup::gradient_scale::extract_spatial_gradient_scales_area_managed(
            smooth.as_ref().unwrap_or(&view),rrrah_dedup::pyramid::PyramidPolicy {local,max_levels:g.search.max_levels,max_total_pixels:g.search.max_total_pixels,max_total_features:g.search.max_total_features},
            &factors,g.max_total_gradient_samples,policy.recipe,(policy.spatial.columns,policy.spatial.rows,policy.spatial.max_per_cell),128000000,&budget,||false);
        match result {Ok(batch)=>{println!("extract {radius:?} count={} capacity={} used={} peak={}",batch.len(),batch.capacity_bytes(),budget.used(),budget.peak());batches.push(batch)},Err(e)=>{println!("extract {radius:?}: {e:?} used={} peak={}",budget.used(),budget.peak());break}}
    }
    drop(batches);drop(image);drop(held);assert_eq!(budget.used(),0);
    println!("released used={} peak={}",budget.used(),budget.peak());
}
