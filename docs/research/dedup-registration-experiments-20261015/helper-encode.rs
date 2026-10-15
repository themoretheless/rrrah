fn encode_srgb(linear: f64) -> f64 {
    if linear.abs() <= 0.003_130_8 {
        return linear * 12.92;
    }
    (1.055 * linear.abs().powf(1. / 2.4) - 0.055).copysign(linear)
}
