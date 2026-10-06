fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    let b = std::fs::read(&a[1])?;
    let image = openreadout_jpegxr::decode(&b, 512 << 20)?;
    std::fs::write(&a[2], &image.data)?;
    println!(
        "{} {} {} {:?} {}",
        image.width, image.height, image.channels, image.sample, image.bgr
    );
    Ok(())
}
