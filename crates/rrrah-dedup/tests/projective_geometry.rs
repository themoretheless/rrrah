use rrrah_dedup::geometry::{fit_projective_four, Correspondence, GeometryError, ProjectiveTransform};
fn fixture() -> [Correspondence; 4] {
    [[0.0, 0.0], [10.0, 0.0], [0.0, 10.0], [10.0, 10.0]].map(|[x, y]| Correspondence {
        source: [x, y],
        target: [
            (2.0 * x + 3.0) / (1.0 + x / 100.0 + y / 200.0),
            (3.0 * y - 2.0) / (1.0 + x / 100.0 + y / 200.0),
        ],
    })
}
#[test]
fn perspective_mapping_and_permutations() {
    let p = fixture();
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    if a == b || a == c || a == d || b == c || b == d || c == d {
                        continue;
                    }
                    let transform = fit_projective_four(&[p[a], p[b], p[c], p[d]], || false).unwrap();
                    for [x, y] in [[0.0, 0.0], [4.0, 7.0], [10.0, 10.0], [-2.0, 3.0]] {
                        let actual = transform.apply([x, y]).unwrap();
                        let den = 1.0 + x / 100.0 + y / 200.0;
                        assert!((actual[0] - (2.0 * x + 3.0) / den).abs() < 1e-10);
                        assert!((actual[1] - (3.0 * y - 2.0) / den).abs() < 1e-10);
                    }
                }
            }
        }
    }
}
#[test]
fn invalid_horizon_and_cancel_checkpoints() {
    let p = fixture();
    let calls = std::cell::Cell::new(0);
    fit_projective_four(&p, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    for checkpoint in 1..=calls.get() {
        let call = std::cell::Cell::new(0);
        assert_eq!(
            fit_projective_four(&p, || {
                call.set(call.get() + 1);
                call.get() == checkpoint
            }),
            Err(GeometryError::Cancelled)
        );
        fit_projective_four(&p, || false).unwrap();
    }
    let line = std::array::from_fn(|i| Correspondence {
        source: [i as f64, 0.0],
        target: [i as f64, 1.0],
    });
    assert_eq!(fit_projective_four(&line, || false), Err(GeometryError::Invalid));
    let mut bad = p;
    bad[0].source[0] = f64::NAN;
    assert_eq!(fit_projective_four(&bad, || false), Err(GeometryError::Invalid));
    let horizon = ProjectiveTransform {
        matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
    };
    assert_eq!(horizon.apply([0.0, 1.0]), None);
}
#[test]
fn robust_perspective_outliers_budget_and_cancellation() {
    use rrrah_dedup::geometry::{verify_projective, GeometryPolicy};
    let mut points = fixture().to_vec();
    for [x, y] in [[2.0, 3.0], [6.0, 8.0], [7.0, 2.0], [9.0, 6.0]] {
        let den = 1.0 + x / 100.0 + y / 200.0;
        points.push(Correspondence {
            source: [x, y],
            target: [(2.0 * x + 3.0) / den, (3.0 * y - 2.0) / den],
        });
    }
    points.push(Correspondence {
        source: [-5.0, 2.0],
        target: [85.0, -39.0],
    });
    let policy = GeometryPolicy {
        tolerance: 1e-6,
        min_inliers: 8,
        max_points: 9,
        max_hypotheses: 126,
    };
    let evidence = verify_projective(&points, policy, || false).unwrap().unwrap();
    assert_eq!(evidence.inliers, (0..8).collect::<Vec<_>>());
    assert_eq!(evidence.hypotheses, 126);
    assert_eq!(
        verify_projective(
            &points,
            GeometryPolicy {
                max_hypotheses: 125,
                ..policy
            },
            || false
        ),
        Err(GeometryError::Budget)
    );
    let calls = std::cell::Cell::new(0);
    assert_eq!(
        verify_projective(&points, policy, || {
            calls.set(calls.get() + 1);
            calls.get() == 50
        }),
        Err(GeometryError::Cancelled)
    );
    assert_eq!(
        verify_projective(&points, policy, || false).unwrap().unwrap(),
        evidence
    );
}
#[test]
fn perspective_pixels_hdr_edits_horizon_and_cancel() {
    use rrrah_dedup::{
        linear::LinearRgbaView,
        warp::{verify_projective_pixels, WarpError, WarpPolicy},
    };
    let target: Vec<f32> = (0..8)
        .flat_map(|y| (0..8).flat_map(move |x| [x as f32, y as f32, -2.0, 1.0]))
        .collect();
    let source: Vec<f32> = (0..4)
        .flat_map(|y| {
            (0..4).flat_map(move |x| {
                let den = 1.0 + x as f32 / 20.0 + y as f32 / 40.0;
                [(x as f32 + 1.0) / den, (y as f32 + 1.0) / den, -2.0, 1.0]
            })
        })
        .collect();
    let a = LinearRgbaView::new(4, 4, &source, 16, || false).unwrap();
    let b = LinearRgbaView::new(8, 8, &target, 64, || false).unwrap();
    let transform = ProjectiveTransform {
        matrix: [[1.0, 0.0, 1.0], [0.0, 1.0, 1.0], [0.05, 0.025, 1.0]],
    };
    let policy = WarpPolicy {
        tolerance: 1e-5,
        max_source_pixels: 16,
    };
    let calls = std::cell::Cell::new(0);
    let evidence = verify_projective_pixels(&a, &b, transform, policy, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!((evidence.compared_pixels, evidence.matched_pixels), (16, 16));
    for checkpoint in 1..=calls.get() {
        let n = std::cell::Cell::new(0);
        assert_eq!(
            verify_projective_pixels(&a, &b, transform, policy, || {
                n.set(n.get() + 1);
                n.get() == checkpoint
            }),
            Err(WarpError::Cancelled)
        );
    }
    assert_eq!(
        verify_projective_pixels(
            &a,
            &b,
            transform,
            WarpPolicy {
                max_source_pixels: 15,
                ..policy
            },
            || false
        ),
        Err(WarpError::Budget)
    );
    let edited: Vec<f32> = target
        .chunks_exact(4)
        .flat_map(|p| [p[0] + 0.1, p[1], p[2], p[3]])
        .collect();
    let b = LinearRgbaView::new(8, 8, &edited, 64, || false).unwrap();
    assert_eq!(
        verify_projective_pixels(&a, &b, transform, policy, || false)
            .unwrap()
            .matched_pixels,
        0
    );
    let horizon = ProjectiveTransform {
        matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, -1.5]],
    };
    assert_eq!(
        verify_projective_pixels(&a, &b, horizon, policy, || false),
        Err(WarpError::Invalid)
    );
}
#[test]
fn perspective_inverse_grid_catches_unsampled_edit() {
    use rrrah_dedup::{
        linear::LinearRgbaView,
        warp::{verify_projective_bidirectional, verify_projective_pixels, WarpError, WarpPolicy},
    };
    let source = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
    let target: Vec<f32> = (0..5)
        .flat_map(|x| [x as f32 / (5.0 - x as f32 * 0.25), 0.0, 0.0, 1.0])
        .collect();
    let a = LinearRgbaView::new(2, 1, &source, 2, || false).unwrap();
    let b = LinearRgbaView::new(5, 1, &target, 5, || false).unwrap();
    let h = ProjectiveTransform {
        matrix: [[5.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.25, 0.0, 1.0]],
    };
    let policy = WarpPolicy {
        tolerance: 1e-6,
        max_source_pixels: 7,
    };
    for p in [[0.0, 0.0], [0.25, 0.5], [1.0, 1.0]] {
        let roundtrip = h.inverse().unwrap().apply(h.apply(p).unwrap()).unwrap();
        assert!((roundtrip[0] - p[0]).abs() < 1e-10);
        assert!((roundtrip[1] - p[1]).abs() < 1e-10);
    }
    let evidence = verify_projective_bidirectional(&a, &b, h, policy, || false).unwrap();
    assert_eq!(evidence.forward.matched_pixels, 2);
    assert_eq!(evidence.reverse.matched_pixels, 5);
    let mut edited = target;
    edited[8] += 0.1;
    let b = LinearRgbaView::new(5, 1, &edited, 5, || false).unwrap();
    assert_eq!(
        verify_projective_pixels(&a, &b, h, policy, || false)
            .unwrap()
            .matched_pixels,
        2
    );
    assert_eq!(
        verify_projective_bidirectional(&a, &b, h, policy, || false)
            .unwrap()
            .reverse
            .matched_pixels,
        4
    );
    assert_eq!(
        verify_projective_bidirectional(
            &a,
            &b,
            h,
            WarpPolicy {
                max_source_pixels: 6,
                ..policy
            },
            || false
        ),
        Err(WarpError::Budget)
    );
    assert_eq!(
        verify_projective_bidirectional(&a, &b, h, policy, || true),
        Err(WarpError::Cancelled)
    );
}
#[test]
fn full_support_refinement_beats_minimal_hypotheses_on_noisy_points() {
    use rrrah_dedup::geometry::{verify_projective,GeometryPolicy};
    let points:Vec<Correspondence>=[[0.0,0.0],[10.0,0.0],[0.0,10.0],[10.0,10.0],[2.0,3.0],[6.0,8.0],[7.0,2.0],[9.0,6.0]].into_iter().enumerate().map(|(i,[x,y])|{
        let den=1.0+x/100.0+y/200.0;
        let noise=if i%2==0 {0.15} else {-0.15};
        Correspondence{source:[x,y],target:[(2.0*x+3.0)/den+noise,(3.0*y-2.0)/den-noise/2.0]}
    }).collect();
    let mut best=f64::INFINITY;
    for a in 0..8 {for b in a+1..8 {for c in b+1..8 {for d in c+1..8 {
        if let Ok(h)=fit_projective_four(&[points[a],points[b],points[c],points[d]],||false) {
            let residuals:Vec<f64>=points.iter().map(|p|h.apply(p.source).map_or(f64::INFINITY,|q|(q[0]-p.target[0]).powi(2)+(q[1]-p.target[1]).powi(2))).collect();
            if residuals.iter().all(|e|*e<=1.0){best=best.min(residuals.iter().sum());}
        }
    }}}}
    let evidence=verify_projective(&points,GeometryPolicy{tolerance:1.0,min_inliers:8,max_points:8,max_hypotheses:70},||false).unwrap().unwrap();
    assert_eq!(evidence.inliers.len(),8);
    assert!(evidence.squared_error<best,"{} >= {best}",evidence.squared_error);
}

#[test]
fn projective_filter_preserves_strict_edits_and_terminal_cancellation() {
    use rrrah_dedup::{linear::LinearRgbaView,warp::{verify_projective_filtered,ColorFilterPolicy,FilterPolicy,FilterColorSpace,WarpPolicy,WarpError}};
    let source:Vec<f32>=(0..25).flat_map(|_|[0.2,0.3,0.4,1.0]).collect();
    let mut edited=source.clone();edited[12*4]=0.29;
    let a=LinearRgbaView::new(5,5,&source,25,||false).unwrap();
    let b=LinearRgbaView::new(5,5,&edited,25,||false).unwrap();
    let model=ProjectiveTransform{matrix:[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]]};
    let pixels=WarpPolicy{tolerance:0.02,max_source_pixels:50};
    let filter=ColorFilterPolicy{filter:FilterPolicy{radius:1,max_sample_pairs:450},color_space:FilterColorSpace::LinearSrgb};
    let calls=std::cell::Cell::new(0);
    let baseline=verify_projective_filtered(&a,&b,model,pixels,filter,||{calls.set(calls.get()+1);false}).unwrap();
    assert_eq!(baseline.strict.forward.matched_pixels,24);
    assert_eq!(baseline.strict.reverse.matched_pixels,24);
    assert_eq!(baseline.filtered.forward.compared_pixels,9);
    assert_eq!(baseline.filtered.forward.matched_pixels,9);
    assert_eq!(baseline.filtered.reverse.matched_pixels,9);
    let mut insufficient=filter;insufficient.filter.max_sample_pairs=449;
    assert_eq!(verify_projective_filtered(&a,&b,model,pixels,insufficient,||false),Err(WarpError::Budget));
    for checkpoint in 1..=calls.get() {
        let current=std::cell::Cell::new(0);
        assert_eq!(verify_projective_filtered(&a,&b,model,pixels,filter,||{current.set(current.get()+1);current.get()==checkpoint}),Err(WarpError::Cancelled));
    }
    assert_eq!(verify_projective_filtered(&a,&b,model,pixels,filter,||false).unwrap(),baseline);
}

#[test]
fn pixel_registration_reduces_known_offset_and_obeys_terminal_limits() {
 use rrrah_dedup::{linear::LinearRgbaView,warp::{refine_projective_pixels,ProjectiveRegistrationPolicy,WarpError}};
 let data:Vec<f32>=(0..32).flat_map(|y|(0..32).flat_map(move |x|[(((x*13+y*7)%19) as f32)/19.,(((x*3+y*11)%23) as f32)/23.,(((x*17+y*5)%29) as f32)/29.,1.])).collect();
 let image=LinearRgbaView::new(32,32,&data,1024,||false).unwrap();
 let initial=ProjectiveTransform{matrix:[[1.,0.,0.1],[0.,1.,0.1],[0.,0.,1.]]};
 let policy=ProjectiveRegistrationPolicy{radius:0,stride:3,rounds:32,max_sample_pairs:100_000};
 let refined=refine_projective_pixels(&image,&image,initial,policy,||false).unwrap();
 for p in [[12.,12.],[16.,16.],[20.,20.]] {let mapped=refined.apply(p).unwrap();assert!((mapped[0]-p[0]).abs()<0.03 && (mapped[1]-p[1]).abs()<0.03,"{refined:?}");}
 let tiny=ProjectiveRegistrationPolicy{max_sample_pairs:0,..policy};
 assert_eq!(refine_projective_pixels(&image,&image,initial,tiny,||false),Err(WarpError::Budget));
 let short=ProjectiveRegistrationPolicy{rounds:1,..policy};let calls=std::cell::Cell::new(0);
 let expected=refine_projective_pixels(&image,&image,initial,short,||{calls.set(calls.get()+1);false}).unwrap();
 for checkpoint in 1..=calls.get() {let n=std::cell::Cell::new(0);assert_eq!(refine_projective_pixels(&image,&image,initial,short,||{n.set(n.get()+1);n.get()==checkpoint}),Err(WarpError::Cancelled));}
 assert_eq!(refine_projective_pixels(&image,&image,initial,short,||false).unwrap(),expected);
 let window=ProjectiveRegistrationPolicy{radius:1,max_sample_pairs:100_000,..short};
 let calls=std::cell::Cell::new(0);
 let baseline=refine_projective_pixels(&image,&image,initial,window,||{calls.set(calls.get()+1);false}).unwrap();
 for checkpoint in [1,100,calls.get()/2,calls.get()-1,calls.get()] {let n=std::cell::Cell::new(0);assert_eq!(refine_projective_pixels(&image,&image,initial,window,||{n.set(n.get()+1);n.get()==checkpoint}),Err(WarpError::Cancelled));}
 assert_eq!(refine_projective_pixels(&image,&image,initial,window,||false).unwrap(),baseline);
 assert_eq!(refine_projective_pixels(&image,&image,initial,ProjectiveRegistrationPolicy{max_sample_pairs:100,..window},||false),Err(WarpError::Budget));
}

#[test]
fn sampled_projective_recovers_large_outlier_set_with_fixed_work() {
 use rrrah_dedup::geometry::{verify_projective,verify_projective_sampled,GeometryPolicy,ProjectiveSamplingPolicy};
 let mut points=Vec::new();
 for i in 0..200 {
  let x=f64::from(i%20)*7.0;let y=f64::from(i/20)*9.0;
  let den=1.0+x*0.0007-y*0.0003;
  let target=if i<120 {[(1.1*x+0.07*y+3.0)/den,(-0.03*x+0.9*y-2.0)/den]}else{[400.0+x*1.7+y*0.4,-300.0+y*1.4+x*0.2]};
  points.push(Correspondence{source:[x,y],target});
 }
 let policy=GeometryPolicy{tolerance:0.001,min_inliers:100,max_points:200,max_hypotheses:256};
 let sampling=ProjectiveSamplingPolicy{trials:256,seed:0x1234abcd};
 let first=verify_projective_sampled(&points,policy,sampling,||false).unwrap().unwrap();
 assert_eq!(first.inliers,(0..120).collect::<Vec<_>>());assert_eq!(first.hypotheses,256);
 assert_eq!(Some(first),verify_projective_sampled(&points,policy,sampling,||false).unwrap());
 assert!(matches!(verify_projective(&points,policy,||false),Err(GeometryError::Budget)));
 assert!(matches!(verify_projective_sampled(&points,policy,ProjectiveSamplingPolicy{trials:257,..sampling},||false),Err(GeometryError::Budget)));
 assert!(matches!(verify_projective_sampled(&points,policy,ProjectiveSamplingPolicy{seed:0,..sampling},||false),Err(GeometryError::Invalid)));
}

#[test]
fn sampled_projective_cancellation_discards_models_and_retry_is_deterministic() {
 use rrrah_dedup::geometry::{verify_projective_sampled,GeometryPolicy,ProjectiveSamplingPolicy};
 let points=fixture();let policy=GeometryPolicy{tolerance:0.001,min_inliers:4,max_points:4,max_hypotheses:4};let sampling=ProjectiveSamplingPolicy{trials:4,seed:17};
 let calls=std::cell::Cell::new(0);
 let baseline=verify_projective_sampled(&points,policy,sampling,||{calls.set(calls.get()+1);false}).unwrap().unwrap();
 for stop in 1..=calls.get(){let current=std::cell::Cell::new(0);assert!(matches!(verify_projective_sampled(&points,policy,sampling,||{current.set(current.get()+1);current.get()==stop}),Err(GeometryError::Cancelled)),"checkpoint {stop}");}
 assert_eq!(Some(baseline),verify_projective_sampled(&points,policy,sampling,||false).unwrap());
}

#[test]
#[allow(clippy::cast_possible_truncation)]
fn photometric_registration_keeps_geometry_under_affine_encoded_light_change() {
 use rrrah_dedup::{linear::LinearRgbaView,warp::{refine_projective_pixels_photometric,PhotometricPolicy,ProjectiveRegistrationPolicy,WarpPolicy,WarpError}};
 let mut source=Vec::new();let mut target=Vec::new();
 let linear=|e:f64|if e<=0.04045 {e/12.92}else{((e+0.055)/1.055).powf(2.4)};
 for y in 0..48 {for x in 0..48 {for c in 0..3 {
  let e=0.1+0.7*f64::from((x*17+y*29+c*11+x*y*3)%101)/100.;source.push(linear(e) as f32);target.push(linear(0.6*e+0.1) as f32);
 }source.push(1.);target.push(1.);}}
 let a=LinearRgbaView::new(48,48,&source,2304,||false).unwrap();let b=LinearRgbaView::new(48,48,&target,2304,||false).unwrap();
 let h=ProjectiveTransform{matrix:[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]]};
 let p=ProjectiveRegistrationPolicy{radius:0,stride:2,rounds:8,max_sample_pairs:2_000_000};
 let fit=PhotometricPolicy{residual:WarpPolicy{tolerance:0.03,max_source_pixels:2304},minimum_samples:16,minimum_variance:0.0001,minimum_gain:0.5,maximum_gain:2.,maximum_offset:0.2};
 let calls=std::cell::Cell::new(0);let refined=refine_projective_pixels_photometric(&a,&b,h,p,fit,||{calls.set(calls.get()+1);false}).unwrap();
 for point in [[0.,0.],[47.,0.],[0.,47.],[47.,47.],[24.,24.]] {let actual=refined.apply(point).unwrap();assert!((actual[0]-point[0]).hypot(actual[1]-point[1])<0.05,"{actual:?}");}
 for stop in [1,calls.get()/2,calls.get()] {let current=std::cell::Cell::new(0);assert!(matches!(refine_projective_pixels_photometric(&a,&b,h,p,fit,||{current.set(current.get()+1);current.get()==stop}),Err(WarpError::Cancelled)));}
 assert!(matches!(refine_projective_pixels_photometric(&a,&b,h,ProjectiveRegistrationPolicy{max_sample_pairs:0,..p},fit,||false),Err(WarpError::Budget)));
 assert_eq!(refined,refine_projective_pixels_photometric(&a,&b,h,p,fit,||false).unwrap());
 use rrrah_dedup::warp::{refine_projective_pixels_anchored,ProjectiveRegistrationTrustPolicy};
 let trust=ProjectiveRegistrationTrustPolicy{registration:ProjectiveRegistrationPolicy{rounds:32,..p},photometric:fit,maximum_corner_shift:0.4};
 let initial=ProjectiveTransform{matrix:[[1.,0.,0.1],[0.,1.,-0.1],[0.,0.,1.]]};
 let anchored=refine_projective_pixels_anchored(&a,&b,initial,trust,||false).unwrap();
 let inverse=anchored.inverse().unwrap();let original=initial.inverse().unwrap();
 for point in [[0.,0.],[47.,0.],[0.,47.],[47.,47.],[24.,24.]] {let actual=anchored.apply(point).unwrap();assert!((actual[0]-point[0]).hypot(actual[1]-point[1])<0.05,"anchored offset recovery {actual:?}");}
 for point in [[0.,0.],[47.,0.],[0.,47.],[47.,47.]] {let actual=inverse.apply(point).unwrap();let before=original.apply(point).unwrap();assert!((actual[0]-before[0]).hypot(actual[1]-before[1])<=0.4000001);}
 let frozen=refine_projective_pixels_anchored(&a,&b,initial,ProjectiveRegistrationTrustPolicy{maximum_corner_shift:0.,..trust},||false).unwrap();
 for point in [[0.,0.],[47.,47.]] {let actual=frozen.apply(point).unwrap();let before=initial.apply(point).unwrap();assert!((actual[0]-before[0]).hypot(actual[1]-before[1])<1e-9);}
 assert!(matches!(refine_projective_pixels_anchored(&a,&b,h,ProjectiveRegistrationTrustPolicy{maximum_corner_shift:f64::NAN,..trust},||false),Err(WarpError::Invalid)));
 assert!(matches!(refine_projective_pixels_anchored(&a,&b,h,trust,||true),Err(WarpError::Cancelled)));
}

#[test]
fn perspective_photometric_fit_preserves_unfitted_errors_and_limits() {
    use rrrah_dedup::{linear::LinearRgbaView, warp::{verify_projective_photometric_filtered,
        ColorFilterPolicy, FilterPolicy, FilterColorSpace, PhotometricPolicy,
        PhotometricFitMode, WarpPolicy, WarpError}};
    // Analytic fields and inverse are independent of the library's resampler.
    let field = |x:f64,y:f64| [0.1+x*0.008+y*0.003,0.2+x*0.002+y*0.009,0.15+x*0.005+y*0.006];
    let mut source=Vec::new(); let mut target=Vec::new();
    let gains=[0.7,1.1,0.8]; let offsets=[0.04,0.02,0.06];
    for y in 0..32 {for x in 0..32 {
        source.extend(field(f64::from(x),f64::from(y)).map(|v|v as f32));source.push(1.0);
        let den=1.0-0.002*f64::from(x);
        let values=field(f64::from(x)/den,f64::from(y)/den);
        for c in 0..3 {target.push((gains[c]*values[c]+offsets[c]) as f32);}target.push(1.0);
    }}
    let a=LinearRgbaView::new(32,32,&source,1024,||false).unwrap();
    let b=LinearRgbaView::new(32,32,&target,1024,||false).unwrap();
    let model=ProjectiveTransform{matrix:[[1.0,0.0,0.0],[0.0,1.0,0.0],[0.002,0.0,1.0]]};
    let photo=PhotometricPolicy{residual:WarpPolicy{tolerance:0.001,max_source_pixels:2048},
        minimum_samples:100,minimum_variance:1e-5,minimum_gain:0.5,maximum_gain:2.0,maximum_offset:0.1};
    let filter=ColorFilterPolicy{filter:FilterPolicy{radius:1,max_sample_pairs:2048*9*3},color_space:FilterColorSpace::LinearSrgb};
    let calls=std::cell::Cell::new(0);
    let evidence=verify_projective_photometric_filtered(&a,&b,model,photo,filter,
        PhotometricFitMode::RejectOutsidePolicy,||{calls.set(calls.get()+1);false}).unwrap();
    for c in 0..3 {assert!((evidence.fitted.forward.gain[c]-gains[c]).abs()<0.002);
        assert!((evidence.fitted.forward.offset[c]-offsets[c]).abs()<0.001);}
    for fitted in [&evidence.fitted.forward,&evidence.fitted.reverse] {
        assert!(fitted.pixels.compared_pixels>700);
        assert_eq!(fitted.pixels.matched_pixels,fitted.pixels.compared_pixels);
    }
    assert!(evidence.unfitted.strict.forward.matched_pixels<evidence.unfitted.strict.forward.compared_pixels/10);
    assert!(evidence.unfitted.filtered.forward.matched_pixels<evidence.unfitted.filtered.forward.compared_pixels/10);
    for checkpoint in [1,calls.get()/2,calls.get()] {
        let count=std::cell::Cell::new(0);
        assert_eq!(verify_projective_photometric_filtered(&a,&b,model,photo,filter,
            PhotometricFitMode::RejectOutsidePolicy,||{count.set(count.get()+1);count.get()==checkpoint}),Err(WarpError::Cancelled));
    }
    assert_eq!(verify_projective_photometric_filtered(&a,&b,model,photo,
        ColorFilterPolicy{filter:FilterPolicy{max_sample_pairs:2048*9*3-1,..filter.filter},..filter},
        PhotometricFitMode::RejectOutsidePolicy,||false),Err(WarpError::Budget));
    let flat:Vec<f32>=(0..1024).flat_map(|_|[0.3,0.4,0.5,1.0]).collect();
    let flat=LinearRgbaView::new(32,32,&flat,1024,||false).unwrap();
    assert!(matches!(verify_projective_photometric_filtered(&flat,&flat,model,photo,filter,
        PhotometricFitMode::RejectOutsidePolicy,||false),
        Err(WarpError::Fit(rrrah_dedup::warp::PhotometricFitFailure::LowVariance{..}))));
    assert!(matches!(verify_projective_photometric_filtered(&a,&b,model,
        PhotometricPolicy{minimum_gain:1.0,maximum_gain:1.0,maximum_offset:0.0,..photo},filter,
        PhotometricFitMode::RejectOutsidePolicy,||false),
        Err(WarpError::Fit(rrrah_dedup::warp::PhotometricFitFailure::OutsidePolicy{..}))));
    let bounded=verify_projective_photometric_filtered(&a,&b,model,
        PhotometricPolicy{minimum_gain:1.0,maximum_gain:1.0,maximum_offset:0.0,..photo},filter,
        PhotometricFitMode::ConstrainedLeastSquares,||false).unwrap();
    assert_eq!(bounded.fitted.forward.gain,[1.0;3]);
    assert_eq!(bounded.fitted.forward.offset,[0.0;3]);
    assert!(bounded.fitted.forward.pixels.matched_pixels<bounded.fitted.forward.pixels.compared_pixels/10);
    let horizon=ProjectiveTransform{matrix:[[1.0,0.0,0.0],[0.0,1.0,0.0],[-0.1,0.0,1.0]]};
    assert_eq!(verify_projective_photometric_filtered(&a,&b,horizon,photo,filter,
        PhotometricFitMode::RejectOutsidePolicy,||false),Err(WarpError::Invalid));
    let transparent=vec![0.0;32*32*4];
    let transparent=LinearRgbaView::new(32,32,&transparent,1024,||false).unwrap();
    assert!(matches!(verify_projective_photometric_filtered(&transparent,&transparent,model,photo,filter,
        PhotometricFitMode::RejectOutsidePolicy,||false),
        Err(WarpError::Fit(rrrah_dedup::warp::PhotometricFitFailure::InsufficientSamples{observed:0,..}))));
    let mut edited=target.clone();edited[(16*32+16)*4+3]=0.4;
    let edited=LinearRgbaView::new(32,32,&edited,1024,||false).unwrap();
    let changed=verify_projective_photometric_filtered(&a,&edited,model,photo,filter,
        PhotometricFitMode::RejectOutsidePolicy,||false).unwrap();
    assert!(changed.fitted.forward.pixels.matched_pixels<changed.fitted.forward.pixels.compared_pixels);
    assert!(changed.fitted.reverse.pixels.matched_pixels<changed.fitted.reverse.pixels.compared_pixels);
    assert_eq!(verify_projective_photometric_filtered(&a,&b,model,photo,filter,
        PhotometricFitMode::RejectOutsidePolicy,||false).unwrap(),evidence);
}

#[test]
fn registration_portfolio_admits_shared_work_and_discards_partial_models_on_cancel() {
 use rrrah_dedup::{linear::LinearRgbaView,warp::{refine_projective_pixels_candidates,ProjectiveRegistrationPortfolioPolicy,ProjectiveRegistrationTrustPolicy,ProjectiveRegistrationPolicy,PhotometricPolicy,WarpPolicy,WarpError}};
 let data:Vec<f32>=(0..32).flat_map(|y|(0..32).flat_map(move|x|[(((x*7+y*13)%31) as f32)/31.,(((x*3+y*11)%23) as f32)/23.,(((x*17+y*5)%29) as f32)/29.,1.])).collect();
 let image=LinearRgbaView::new(32,32,&data,1024,||false).unwrap();
 let initial=ProjectiveTransform{matrix:[[1.,0.,0.1],[0.,1.,0.1],[0.,0.,1.]]};
 let lane=ProjectiveRegistrationPolicy{radius:0,stride:3,rounds:16,max_sample_pairs:100_000};
 let trust=ProjectiveRegistrationTrustPolicy{registration:lane,photometric:PhotometricPolicy{residual:WarpPolicy{tolerance:0.03,max_source_pixels:2048},minimum_samples:16,minimum_variance:1e-5,minimum_gain:0.2,maximum_gain:5.,maximum_offset:0.1},maximum_corner_shift:1.};
 let policy=ProjectiveRegistrationPortfolioPolicy{anchored:trust,unanchored:lane,max_sample_pairs:200_000};
 let calls=std::cell::Cell::new(0);let result=refine_projective_pixels_candidates(&image,&image,initial,policy,||{calls.set(calls.get()+1);false}).unwrap();
 for model in [result.anchored,result.unanchored] {let p=model.apply([16.,16.]).unwrap();assert!((p[0]-16.).hypot(p[1]-16.)<0.05);}
 for checkpoint in [1,calls.get()/2,calls.get()-1,calls.get()] {
  let count=std::cell::Cell::new(0);assert_eq!(refine_projective_pixels_candidates(&image,&image,initial,policy,||{count.set(count.get()+1);count.get()==checkpoint}),Err(WarpError::Cancelled));
 }
 let observed=std::cell::Cell::new(0);assert_eq!(refine_projective_pixels_candidates(&image,&image,initial,ProjectiveRegistrationPortfolioPolicy{max_sample_pairs:199_999,..policy},||{observed.set(observed.get()+1);false}),Err(WarpError::Budget));assert_eq!(observed.get(),0);
 assert_eq!(refine_projective_pixels_candidates(&image,&image,initial,ProjectiveRegistrationPortfolioPolicy{unanchored:ProjectiveRegistrationPolicy{max_sample_pairs:0,..lane},..policy},||false),Err(WarpError::Budget));
 assert_eq!(refine_projective_pixels_candidates(&image,&image,initial,policy,||false).unwrap(),result);
}

#[test]
fn registration_candidates_keep_independent_full_grid_evidence_and_shared_limits() {
 use rrrah_dedup::{linear::LinearRgbaView,warp::{verify_projective_candidates_filtered,ProjectiveRegistrationCandidates,WarpPolicy,ColorFilterPolicy,FilterPolicy,FilterColorSpace,WarpError}};
 let data:Vec<f32>=(0..32).flat_map(|y|(0..32).flat_map(move|x|[(((x*7+y*13)%31) as f32)/31.,(((x*3+y*11)%23) as f32)/23.,(((x*17+y*5)%29) as f32)/29.,1.])).collect();
 let image=LinearRgbaView::new(32,32,&data,1024,||false).unwrap();
 let models=ProjectiveRegistrationCandidates{anchored:ProjectiveTransform{matrix:[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]]},unanchored:ProjectiveTransform{matrix:[[1.,0.,5.],[0.,1.,0.],[0.,0.,1.]]}};
 let pixels=WarpPolicy{tolerance:0.001,max_source_pixels:4096};
 let filter=ColorFilterPolicy{filter:FilterPolicy{radius:1,max_sample_pairs:4096*9},color_space:FilterColorSpace::LinearSrgb};
 let calls=std::cell::Cell::new(0);let evidence=verify_projective_candidates_filtered(&image,&image,models.clone(),pixels,filter,||{calls.set(calls.get()+1);false}).unwrap();
 assert_eq!(evidence.anchored.strict.forward.matched_pixels,1024);assert_eq!(evidence.anchored.strict.reverse.matched_pixels,1024);
 assert_eq!(evidence.anchored.filtered.forward.matched_pixels,900);assert_eq!(evidence.anchored.filtered.reverse.matched_pixels,900);
 assert!(evidence.unanchored.filtered.forward.compared_pixels>700);assert!(evidence.unanchored.filtered.forward.matched_pixels<evidence.unanchored.filtered.forward.compared_pixels/10);
 for checkpoint in [1,calls.get()/2,calls.get()-1,calls.get()] {
  let count=std::cell::Cell::new(0);assert_eq!(verify_projective_candidates_filtered(&image,&image,models.clone(),pixels,filter,||{count.set(count.get()+1);count.get()==checkpoint}),Err(WarpError::Cancelled));
 }
 assert_eq!(verify_projective_candidates_filtered(&image,&image,models.clone(),WarpPolicy{max_source_pixels:4095,..pixels},filter,||false),Err(WarpError::Budget));
 assert_eq!(verify_projective_candidates_filtered(&image,&image,models.clone(),pixels,ColorFilterPolicy{filter:FilterPolicy{max_sample_pairs:4096*9-1,..filter.filter},..filter},||false),Err(WarpError::Budget));
 assert_eq!(verify_projective_candidates_filtered(&image,&image,models,pixels,filter,||false).unwrap(),evidence);
}
