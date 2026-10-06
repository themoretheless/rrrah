//! Test-only independent Aseprite decoder; built outside the runtime workspace.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let file = asefile::AsepriteFile::read_file(std::path::Path::new(&args[1]))?;
    let index: u32 = args[2].parse()?;
    let frame = file.frame(index);
    std::fs::write(&args[3], frame.image().as_raw())?;
    println!("{} {} {} {}", file.width(), file.height(), file.num_frames(), frame.duration());
    Ok(())
}
