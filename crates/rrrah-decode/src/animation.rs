//! Shared linear-light straight-alpha animation composition.
pub(crate) fn over(dst: &mut [f32], src: &[f32]) {
    let a = f64::from(src[3]);
    let d = f64::from(dst[3]);
    if a == 0. {
        return;
    }
    let alpha = a + d * (1. - a);
    for c in 0..3 {
        dst[c] = ((f64::from(src[c]) * a + f64::from(dst[c]) * d * (1. - a)) / alpha) as f32;
    }
    dst[3] = alpha as f32;
}
