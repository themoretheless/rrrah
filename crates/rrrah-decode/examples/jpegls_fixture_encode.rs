//! Test-only CharLS encoder for CC0 source samples; not a viewer dependency.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    if a.len() != 9 {
        return Err("RAW OUTPUT WIDTH HEIGHT BITS COMPONENTS ILV NEAR".into());
    }
    let mut codec = charls::CharLS::default();
    codec.set_interleave_mode(match a[7].as_str() {
        "0" => charls::InterleaveMode::None,
        "1" => charls::InterleaveMode::Line,
        "2" => charls::InterleaveMode::Sample,
        _ => return Err("ILV".into()),
    })?;
    let info = charls::FrameInfo {
        width: a[3].parse()?,
        height: a[4].parse()?,
        bits_per_sample: a[5].parse()?,
        component_count: a[6].parse()?,
    };
    let encoded = codec.encode(info, a[8].parse()?, &std::fs::read(&a[1])?)?;
    std::fs::write(&a[2], encoded)?;
    Ok(())
}
