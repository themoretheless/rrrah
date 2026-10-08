//! Test-only native component algebra. Callers must convert group spaces before
//! crossing a boundary; matching component counts never imply matching profiles.
use hayro_interpret::BlendMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Model {
    Gray,
    Rgb,
    Cmyk,
}
impl Model {
    pub(super) fn channels(self) -> usize {
        match self {
            Self::Gray => 1,
            Self::Rgb => 3,
            Self::Cmyk => 4,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Pixel {
    model: Model,
    // Additive premultiplied coordinates; CMYK components are complemented.
    color: [f64; 4],
    alpha: f64,
}
impl Pixel {
    pub(super) fn model(self) -> Model {
        self.model
    }
    pub(super) fn alpha(self) -> f64 {
        self.alpha
    }
    pub(super) fn from_native(model: Model, components: &[f64], alpha: f64) -> Option<Self> {
        if components.len() != model.channels()
            || !alpha.is_finite()
            || !(0. ..=1.).contains(&alpha)
            || components
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return None;
        }
        let mut color = [0.; 4];
        for (out, &value) in color.iter_mut().zip(components) {
            *out = alpha * if model == Model::Cmyk { 1. - value } else { value };
        }
        Some(Self { model, color, alpha })
    }
    pub(super) fn valid(self) -> bool {
        self.alpha.is_finite()
            && self.alpha >= -1e-12
            && self.alpha <= 1. + 1e-12
            && self.color[..self.model.channels()]
                .iter()
                .all(|&v| v.is_finite() && v >= -1e-12 && v <= self.alpha + 1e-12)
            && self.color[self.model.channels()..].iter().all(|&v| v == 0.)
    }
    pub(super) fn native_components(self) -> Option<[f64; 4]> {
        if !self.valid() {
            return None;
        }
        let mut output = [0.; 4];
        if self.alpha == 0. {
            return Some(output);
        }
        for (i, out) in output[..self.model.channels()].iter_mut().enumerate() {
            let additive = self.color[i] / self.alpha;
            *out = if self.model == Model::Cmyk {
                1. - additive
            } else {
                additive
            };
        }
        Some(output)
    }
    pub(super) fn scale_opacity(self, factor: f64) -> Option<Self> {
        if !self.valid() || !factor.is_finite() || !(0. ..=1.).contains(&factor) {
            return None;
        }
        Some(Self {
            color: self.color.map(|v| v * factor),
            alpha: self.alpha * factor,
            ..self
        })
    }
    pub(super) fn composite(
        previous: Self,
        backdrop: Self,
        source: Self,
        shape: f64,
        mode: BlendMode,
    ) -> Option<Self> {
        if previous.model != backdrop.model
            || source.model != previous.model
            || !previous.valid()
            || !backdrop.valid()
            || !source.valid()
            || !shape.is_finite()
            || !(0. ..=1. + 1e-12).contains(&shape)
            || source.alpha > shape + 1e-12
            || !matches!(
                mode,
                BlendMode::Normal | BlendMode::Multiply | BlendMode::Screen | BlendMode::HardLight
            )
        {
            return None;
        }
        let (a, b) = (source.alpha, backdrop.alpha);
        let mut result = Self {
            model: source.model,
            color: [0.; 4],
            alpha: a + (shape - a) * b + (1. - shape) * previous.alpha,
        };
        for c in 0..source.model.channels() {
            let cs = if a == 0. { 0. } else { source.color[c] / a };
            let cb = if b == 0. { 0. } else { backdrop.color[c] / b };
            let blend = match mode {
                BlendMode::Normal => cs,
                BlendMode::Multiply => cb * cs,
                BlendMode::Screen => cb + cs - cb * cs,
                BlendMode::HardLight if cs <= 0.5 => 2. * cb * cs,
                BlendMode::HardLight => 1. - 2. * (1. - cb) * (1. - cs),
                _ => unreachable!(),
            };
            result.color[c] = a * ((1. - b) * cs + b * blend)
                + (shape - a) * backdrop.color[c]
                + (1. - shape) * previous.color[c];
        }
        result.valid().then_some(result)
    }
    pub(super) fn remove_backdrop(self, initial: Self, group_alpha: f64) -> Option<Self> {
        if self.model != initial.model
            || !self.valid()
            || !initial.valid()
            || !group_alpha.is_finite()
            || !(0. ..=1. + 1e-12).contains(&group_alpha)
            || (self.alpha - (group_alpha + (1. - group_alpha) * initial.alpha)).abs() > 1e-12
        {
            return None;
        }
        let mut output = Self {
            model: self.model,
            color: [0.; 4],
            alpha: group_alpha,
        };
        for c in 0..self.model.channels() {
            let value = self.color[c] - (1. - group_alpha) * initial.color[c];
            if value < -1e-12 || value > group_alpha + 1e-12 {
                return None;
            }
            output.color[c] = value.clamp(0., group_alpha);
        }
        output.valid().then_some(output)
    }
}

#[test]
fn independent_cmyk_normal_overlap_and_transparent_knockout() {
    let make = |c: &[f64], a| Pixel::from_native(Model::Cmyk, c, a).unwrap();
    let clear = make(&[0.; 4], 0.);
    let cyan = make(&[1., 0., 0., 0.], 0.5);
    let magenta = make(&[0., 1., 0., 0.], 0.5);
    let first = Pixel::composite(clear, clear, cyan, 1., BlendMode::Normal).unwrap();
    let output = Pixel::composite(first, first, magenta, 1., BlendMode::Normal).unwrap();
    assert_eq!(output.alpha, 0.75);
    for (v, expected) in output
        .native_components()
        .unwrap()
        .iter()
        .zip([1. / 3., 2. / 3., 0., 0.])
    {
        assert!((v - expected).abs() < 1e-14);
    }
    let previous = make(&[0., 1., 0., 0.], 1.);
    let initial = make(&[0.; 4], 1.);
    let transparent = make(&[1., 0., 0., 0.], 0.);
    let knocked = Pixel::composite(previous, initial, transparent, 1., BlendMode::Normal).unwrap();
    assert_eq!(knocked.native_components().unwrap(), [0.; 4]);
    let retained = Pixel::composite(previous, initial, transparent, 0., BlendMode::Normal).unwrap();
    assert_eq!(retained.native_components().unwrap(), [0., 1., 0., 0.]);
    let partial = Pixel::composite(
        previous,
        initial,
        make(&[1., 0., 0., 0.], 0.25),
        0.5,
        BlendMode::Normal,
    )
    .unwrap();
    assert_eq!(partial.native_components().unwrap(), [0.25, 0.5, 0., 0.]);
}
#[test]
fn subtractive_modes_complement_all_four_channels() {
    let backdrop = Pixel::from_native(Model::Cmyk, &[0.2; 4], 1.).unwrap();
    let source = Pixel::from_native(Model::Cmyk, &[0.8; 4], 1.).unwrap();
    for (mode, expected) in [
        (BlendMode::Multiply, 0.84),
        (BlendMode::Screen, 0.16),
        (BlendMode::HardLight, 0.68),
    ] {
        let pixel = Pixel::composite(backdrop, backdrop, source, 1., mode).unwrap();
        for v in pixel.native_components().unwrap() {
            assert!((v - expected).abs() < 1e-14);
        }
    }
}
#[test]
fn native_group_removal_and_opacity_preserve_unquantized_contribution() {
    let initial = Pixel::from_native(Model::Cmyk, &[1., 0., 0., 0.], 1.).unwrap();
    let source = Pixel::from_native(Model::Cmyk, &[0., 1., 0., 0.], 0.5).unwrap();
    let current = Pixel::composite(initial, initial, source, 1., BlendMode::Normal).unwrap();
    let removed = current.remove_backdrop(initial, 0.5).unwrap();
    assert_eq!(removed.native_components().unwrap(), [0., 1., 0., 0.]);
    let scaled = removed.scale_opacity(0.37).unwrap();
    assert_eq!(scaled.alpha, 0.185);
    assert_eq!(scaled.native_components().unwrap(), [0., 1., 0., 0.]);
}
#[test]
fn gray_and_rgb_match_independent_scalar_and_existing_rgb_algebra() {
    let gray = Pixel::from_native(Model::Gray, &[0.2], 1.).unwrap();
    let white = Pixel::from_native(Model::Gray, &[1.], 0.5).unwrap();
    assert_eq!(
        Pixel::composite(gray, gray, white, 1., BlendMode::Normal)
            .unwrap()
            .native_components()
            .unwrap()[0],
        0.6
    );
    let p = Pixel::from_native(Model::Rgb, &[0.1, 0.7, 0.4], 0.7).unwrap();
    let b = Pixel::from_native(Model::Rgb, &[0.5, 0.2, 0.9], 0.4).unwrap();
    let s = Pixel::from_native(Model::Rgb, &[0.8, 0.1, 0.6], 0.3).unwrap();
    let rgba = |v: Pixel| [v.color[0], v.color[1], v.color[2], v.alpha];
    for shape in [0.5, 1.] {
        for mode in [
            BlendMode::Normal,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::HardLight,
        ] {
            let got = Pixel::composite(p, b, s, shape, mode).unwrap();
            assert_eq!(
                rgba(got),
                super::composite_precise_pixel(rgba(p), rgba(b), rgba(s), shape, mode).unwrap()
            );
        }
    }
}
#[test]
fn invalid_native_samples_spaces_and_modes_refuse() {
    assert!(Pixel::from_native(Model::Cmyk, &[0.; 3], 1.).is_none());
    assert!(Pixel::from_native(Model::Gray, &[f64::NAN], 1.).is_none());
    assert!(Pixel::from_native(Model::Gray, &[0.5], f64::INFINITY).is_none());
    let gray = Pixel::from_native(Model::Gray, &[0.5], 1.).unwrap();
    let rgb = Pixel::from_native(Model::Rgb, &[0.5; 3], 1.).unwrap();
    assert!(Pixel::composite(gray, gray, rgb, 1., BlendMode::Normal).is_none());
    assert!(Pixel::composite(gray, gray, gray, 0.5, BlendMode::Normal).is_none());
    assert!(Pixel::composite(gray, gray, gray, 1., BlendMode::Difference).is_none());
    let black = Pixel::from_native(Model::Gray, &[0.], 1.).unwrap();
    assert!(black.remove_backdrop(gray, 0.5).is_none());
}
