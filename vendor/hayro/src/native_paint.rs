//! Test-only retained original PDF paint. Pixel blending still uses a separate
//! compatibility display sample until native group conversion is connected.
use hayro_interpret::color::{Color, ColorSpace};

pub(super) struct NativePaint {
    pub(super) color_space: ColorSpace,
    pub(super) components: Vec<f32>,
    pub(super) opacity: f32,
    pub(super) conversion_functions: hayro_interpret::color::DeviceConversionFunctions,
    pub(super) display: [u8; 4],
    // Components and retained profile reference drop before the credit.
    _credit: Box<dyn std::any::Any + Send + Sync>,
}
impl NativePaint {
    /// Construct a native sample using coordinate identity or the explicit
    /// device/calibrated display policy to DeviceRGB. Other boundaries require
    /// a transform; calibrated group conformance is not established here.
    pub(super) fn pixel_in_space(
        &self,
        destination: &ColorSpace,
        shape: f64,
    ) -> Option<super::native_blend::Pixel> {
        use super::native_blend::{Model, Pixel};
        use hayro_interpret::color::BlendingModel;
        if !shape.is_finite() || !(0. ..=1.).contains(&shape) {
            return None;
        }
        let model = match destination.blending_model()? {
            BlendingModel::Gray => Model::Gray,
            BlendingModel::Rgb => Model::Rgb,
            BlendingModel::Cmyk => Model::Cmyk,
        };
        let mut components = [0.; 4];
        let converted;
        let input = if self.color_space.shares_blending_coordinates(destination) {
            &self.components[..]
        } else {
            converted = self.color_space.device_sample_to_rgb(destination, &self.components)
                .or_else(|| self.color_space.device_sample_to_gray_or_cmyk(destination, &self.components))
                .or_else(|| self.color_space.rgb_sample_to_cmyk_with(destination, &self.components, &self.conversion_functions, &|| false))
                .or_else(|| self.color_space.calibrated_sample_to_rgb(destination, &self.components))?;
            &converted[..model.channels()]
        };
        for (out, &value) in components.iter_mut().zip(input) {
            *out = f64::from(value);
        }
        Pixel::from_native(
            model,
            &components[..model.channels()],
            f64::from(self.opacity) * shape,
        )
    }
    pub(super) fn capture(
        color: &Color,
        cancelled: &dyn Fn() -> bool,
        admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    ) -> Option<Box<Self>> {
        let input = color.components();
        if cancelled()
            || input.is_empty()
            || input.len() > 255
            || input.len() != usize::from(color.color_space().num_components())
            || !color.opacity().is_finite()
            || !(0. ..=1.).contains(&color.opacity())
            || input.iter().any(|v| !v.is_finite())
        {
            return None;
        }
        // Native Lab/spot coordinates may have other ranges. Conversion into
        // a valid blending space must precede bounded [0,1] pixel construction.
        let bytes = size_of::<Self>().checked_add(input.len().checked_mul(size_of::<f32>())?)?;
        let credit = admit(bytes)?;
        if cancelled() {
            return None;
        }
        let mut components = Vec::new();
        components.try_reserve_exact(input.len()).ok()?;
        for &component in input {
            if cancelled() {
                return None;
            }
            components.push(component);
        }
        let retained = Self {
            color_space: color.color_space().clone(),
            components,
            opacity: color.opacity(),
            conversion_functions: color.conversion_functions().clone(),
            display: color.to_rgba().to_rgba8(),
            _credit: credit,
        };
        if cancelled() {
            return None;
        }
        Some(Box::new(retained))
    }
}

#[test]
fn original_paint_credit_and_cancellation_preserve_native_precision() {
    use hayro_interpret::color::AlphaColor;
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let source = Color::from_rgba(AlphaColor::new([0.17, 0.31, 0.53, 0.37]));
    let used = Arc::new(AtomicUsize::new(0));
    let admit = |bytes| {
        used.fetch_add(bytes, Ordering::Relaxed);
        Some(Box::new(Credit(used.clone(), bytes)) as Box<dyn std::any::Any + Send + Sync>)
    };
    let polls = Cell::new(0);
    let paint = NativePaint::capture(
        &source,
        &|| {
            polls.set(polls.get() + 1);
            false
        },
        &admit,
    )
    .unwrap();
    assert_eq!(paint.components, [0.17, 0.31, 0.53]);
    assert_eq!(paint.opacity, 0.37);
    assert_eq!(paint.color_space.num_components(), 3);
    let sample = paint.pixel_in_space(&paint.color_space, 0.5).unwrap();
    assert_eq!(sample.alpha(), f64::from(source.opacity()) * 0.5);
    for (value, &original) in sample
        .native_components()
        .unwrap()
        .iter()
        .zip(source.components())
    {
        assert!((*value - f64::from(original)).abs() < 1e-15);
    }
    assert!(paint.pixel_in_space(&paint.color_space, f64::NAN).is_none());
    assert!(paint.pixel_in_space(&paint.color_space, 1.01).is_none());
    assert_eq!(
        used.load(Ordering::Relaxed),
        size_of::<NativePaint>() + 3 * size_of::<f32>()
    );
    drop(paint);
    assert_eq!(used.load(Ordering::Relaxed), 0);
    for stop in 1..=polls.get() {
        let current = Cell::new(0);
        assert!(
            NativePaint::capture(
                &source,
                &|| {
                    current.set(current.get() + 1);
                    current.get() == stop
                },
                &admit
            )
            .is_none()
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    assert!(NativePaint::capture(&source, &|| false, &|_| None).is_none());
    assert_eq!(used.load(Ordering::Relaxed), 0);
}
