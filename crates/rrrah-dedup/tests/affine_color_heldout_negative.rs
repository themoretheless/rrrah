use rrrah_dedup::affine_color::{AffineColorPolicy, RgbPair, fit_affine_color, validate_affine_color};
#[test]
fn fitted_color_does_not_accept_unrelated_heldout_samples() {
    let samples: Vec<_> = (0..2048)
        .map(|i| {
            let source = [
                ((i * 17) % 251) as f64 / 250.,
                ((i * 31) % 241) as f64 / 240.,
                ((i * 47) % 239) as f64 / 238.,
            ];
            RgbPair {
                source,
                target: source.map(|v| 0.7 * v + 0.05),
            }
        })
        .collect();
    let model = fit_affine_color(
        &samples[..1024],
        AffineColorPolicy {
            minimum_samples: 1000,
            maximum_samples: 20000,
            minimum_variance: 1e-5,
            minimum_relative_pivot: 1e-6,
            maximum_coefficient: 5.,
            maximum_offset: 0.1,
        },
        || false,
    )
    .unwrap();
    let good = validate_affine_color(model, &samples[1024..], 0.03, 20000, || false).unwrap();
    assert_eq!(good.matched, good.samples);
    let unrelated: Vec<_> = samples[1024..]
        .iter()
        .enumerate()
        .map(|(i, p)| RgbPair {
            source: p.source,
            target: samples[(i + 173) % 1024].target,
        })
        .collect();
    let bad = validate_affine_color(model, &unrelated, 0.03, 20000, || false).unwrap();
    assert!(bad.matched < bad.samples / 10);
}
