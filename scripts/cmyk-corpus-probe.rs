use moxcms::{ColorProfile, Layout, TransformOptions};
fn main() {
    let a: Vec<_> = std::env::args().collect();
    let profile = ColorProfile::new_from_slice(&std::fs::read(&a[1]).unwrap()).unwrap();
    let bytes = std::fs::read(&a[2]).unwrap();
    assert_eq!(bytes.len() % 16, 0);
    let input: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let executor = profile
        .create_transform_f32(
            Layout::Rgba,
            &ColorProfile::new_srgb(),
            Layout::Rgb,
            TransformOptions::default(),
        )
        .unwrap();
    let mut output = vec![0.0; input.len() / 4 * 3];
    executor.transform(&input, &mut output).unwrap();
    std::fs::write(
        &a[3],
        output.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>(),
    )
    .unwrap();
}
