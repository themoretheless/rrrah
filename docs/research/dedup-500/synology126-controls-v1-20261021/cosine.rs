// Unchanged cosine_q extracted from pinned upstream semantic.rs; authored tests follow.
pub fn cosine_q(a: &[i8], b: &[i8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0i64, 0i64, 0i64);
    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += x as i64 * y as i64;
        na += x as i64 * x as i64;
        nb += y as i64 * y as i64;
    }
    if na == 0 || nb == 0 {
        return 0.0;
    }
    dot as f32 / ((na as f32).sqrt() * (nb as f32).sqrt())
}

#[test] fn empty_and_mismatched_dimensions() { assert_eq!(cosine_q(&[], &[]),0.0); assert_eq!(cosine_q(&[1], &[1,2]),0.0); }
#[test] fn zero_norm() { assert_eq!(cosine_q(&[0,0], &[1,2]),0.0); }
#[test] fn parallel() { assert!((cosine_q(&[1,2], &[2,4])-1.0).abs()<1e-6); }
#[test] fn orthogonal() { assert_eq!(cosine_q(&[127,0], &[0,127]),0.0); }
#[test] fn opposite() { assert!((cosine_q(&[127,0], &[-127,0])+1.0).abs()<1e-6); }
