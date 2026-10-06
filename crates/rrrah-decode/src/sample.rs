//! Native sample conversion shared by scientific and texture codecs.
pub(crate) fn half_to_f32(bits: u16) -> f32 {
    let sign = (u32::from(bits) & 0x8000) << 16;
    let exponent = (bits >> 10) & 31;
    let fraction = u32::from(bits & 1023);
    match exponent {
        0 => (fraction as f32 * 2f32.powi(-24)).copysign(f32::from_bits(sign)),
        31 => {
            f32::from_bits(sign | 0x7f800000 | (fraction << 13) | if fraction != 0 { 0x00400000 } else { 0 })
        }
        _ => f32::from_bits(sign | ((u32::from(exponent) + 112) << 23) | (fraction << 13)),
    }
}
