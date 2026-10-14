fn main() {
    let decode = |name: &str| {
        let bytes = std::fs::read(format!("tests/fixtures/raster/{name}")).unwrap();
        image::load_from_memory_with_format(&bytes, image::ImageFormat::Avif).unwrap().into_rgba8()
    };
    let straight = decode("avif-alpha-icc.avif");
    let prem = decode("avif-alpha-icc-prem.avif");
    assert_eq!(straight.dimensions(), prem.dimensions());
    assert_eq!(straight.as_raw(), prem.as_raw());
    let (i, p) = prem.pixels().enumerate().find(|(_,p)| p[3] > 0 && p[3] < 255 && p[0] > 0).unwrap();
    println!("image 0.25.10 backend returns byte-identical RGBA for straight and prem controls: {} pixels", prem.pixels().len());
    println!("partial pixel {i}: stored={:?}; straight-alpha interpretation requires RGB scaled by 255/{}", p.0, p[3]);
}
