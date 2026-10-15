fn premultiplied_in_space(
    image: &LinearRgbaView<'_>,
    x: u32,
    y: u32,
    space: FilterColorSpace,
) -> Option<[f64; 4]> {
    let mut p = image.pixel(x, y)?.map(|bits| f64::from(f32::from_bits(bits)));
    if space == FilterColorSpace::EncodedSrgb {
        let red = p[0];
        let green = p[1];
        let blue = p[2];
        p[0] = encode_srgb(red);
        p[1] = if green.to_bits() == red.to_bits() { p[0] } else { encode_srgb(green) };
        p[2] = if blue.to_bits() == red.to_bits() { p[0] }
            else if blue.to_bits() == green.to_bits() { p[1] }
            else { encode_srgb(blue) };
    }
    Some([p[0] * p[3], p[1] * p[3], p[2] * p[3], p[3]])
}
