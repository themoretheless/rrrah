fn premultiplied_in_space(
    image: &LinearRgbaView<'_>,
    x: u32,
    y: u32,
    space: FilterColorSpace,
) -> Option<[f64; 4]> {
    let mut p = image.pixel(x, y)?.map(|bits| f64::from(f32::from_bits(bits)));
    if space == FilterColorSpace::EncodedSrgb {
        for c in &mut p[..3] {
            *c = encode_srgb(*c);
        }
    }
    Some([p[0] * p[3], p[1] * p[3], p[2] * p[3], p[3]])
}
