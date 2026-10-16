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
/// Propagate component discrepancy through fixed-alpha Normal composition.
/// Previous/backdrop bounds use additive premultiplied storage; source bounds
/// use native components before Pixel::from_native. Matching coordinates, fixed
/// alphas/shape and checked0..1native samples are caller obligations. This is
/// not a group/route certificate and never authorizes a color-space conversion.
pub(super) fn normal_premultiplied_error_bound(model:Model,previous:[f64;4],backdrop:[f64;4],
    source:[f64;4],source_alpha:f64,shape:f64,remaining:&mut usize,cancelled:&dyn Fn()->bool,
)->Option<[f64;4]> {
    if cancelled() {return None;}
    *remaining=remaining.checked_sub(1+6*model.channels())?;
    if !source_alpha.is_finite() || !shape.is_finite() || !(0. ..=1.).contains(&shape)
        || !(0. ..=shape).contains(&source_alpha) || (source_alpha>0. && source_alpha<1e-12) {return None;}
    for errors in [&previous,&backdrop,&source] {
        if errors.iter().any(|e|!e.is_finite() || *e<0.)
            || errors[model.channels()..].iter().any(|e|*e!=0.) {return None;}
    }
    let add=|a:f64,b:f64|(a+b).next_up();let mul=|a:f64,b:f64|(a*b).next_up();
    let previous_gain=(1.-shape).next_up();let backdrop_gain=(shape-source_alpha).next_up();
    let mut result=[0.;4];
    for channel in 0..model.channels() {
        if cancelled() {return None;}
        // Exact Normal algebra: alpha*Cs +(shape-alpha)*Cb +(1-shape)*Cp.
        // CMYK complementation changes the sign of source discrepancy, not its
        // absolute gain. Include a conservative absolute envelope for checked
        // sample premultiplication/complement, ordered Normal arithmetic and
        // its source normalization. Tiny positive alpha refuses above.
        result[channel]=add(add(mul(previous_gain,previous[channel]),mul(backdrop_gain,backdrop[channel])),add(mul(source_alpha,source[channel]),256.*f64::EPSILON));
        if !result[channel].is_finite() {return None;}
    }
    if cancelled() {return None;}Some(result)
}

/// Error operation with fixed, identical alpha/opacity on both compared fields.
/// Removal compares identically clamped results, so it does not prove affinity.
#[derive(Clone,Copy)]
pub(super) enum GroupErrorStep {
    RemoveBackdrop {alpha:f64,initial:[f64;4]},
    ScaleOpacity {factor:f64},
    NativeComponents {alpha:f64},
}
/// Propagate additive premultiplied errors through a same-space group boundary.
/// NativeComponents returns unpremultiplied native-component errors. No variant
/// authorizes color conversion, differing alpha, or an affine integration domain.
pub(super) fn group_error_bound(model:Model,current:[f64;4],step:GroupErrorStep,
    remaining:&mut usize,cancelled:&dyn Fn()->bool)->Option<[f64;4]> {
    if cancelled() {return None;}*remaining=remaining.checked_sub(1+7*model.channels())?;
    let valid=|e:&[f64;4]|e.iter().all(|v|v.is_finite() && *v>=0.) && e[model.channels()..].iter().all(|v|*v==0.);
    if !valid(&current) {return None;}
    match step {
        GroupErrorStep::RemoveBackdrop {alpha,initial}=>{if !alpha.is_finite() || !(0. ..=1.).contains(&alpha) || !valid(&initial) {return None;}},
        GroupErrorStep::ScaleOpacity {factor}=>{if !factor.is_finite() || !(0. ..=1.).contains(&factor) {return None;}},
        GroupErrorStep::NativeComponents {alpha}=>{if !alpha.is_finite() || !(0. ..=1.).contains(&alpha) || (alpha>0. && alpha<1e-12) {return None;}},
    }
    let add=|a:f64,b:f64|(a+b).next_up();let mul=|a:f64,b:f64|(a*b).next_up();
    let mut output=[0.;4];
    for channel in 0..model.channels() {
        if cancelled() {return None;}
        output[channel]=match step {
            // Clamp to the same0..alpha interval is nonexpansive. It may still
            // destroy affinity, which requires a separate no-active-clamp proof.
            GroupErrorStep::RemoveBackdrop {alpha,initial}=>add(add(current[channel],mul((1.-alpha).next_up(),initial[channel])),64.*f64::EPSILON),
            GroupErrorStep::ScaleOpacity {factor}=>if factor==0. {0.} else {add(mul(factor,current[channel]),4.*f64::EPSILON)},
            GroupErrorStep::NativeComponents {alpha}=>if alpha==0. {0.} else {add((current[channel]/alpha).next_up(),8.*f64::EPSILON)},
        };
        if !output[channel].is_finite() {return None;}
    }
    if cancelled() {return None;}Some(output)
}

/// Absolute error of from_native(...).native_components() for supplied unit
/// components and the same positive alpha. Covers CMYK complements, multiply,
/// division and gradual underflow; excludes incoming quotient/source errors.
pub(super) fn native_storage_error_bound(model:Model,alpha:f64,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[f64;4]> {
    if cancelled() {return None;}*work=work.checked_sub(24)?;
    if !alpha.is_finite() || alpha<=0. || alpha>1. {return None;}
    let u=f64::EPSILON;let tiny=f64::from_bits(1);
    let add=|a:f64,b:f64|(a+b).next_up();let mul=|a:f64,b:f64|(a*b).next_up();
    let mut error=if model==Model::Cmyk {add(u,tiny)} else {0.};
    // Normalize multiplication error by alpha before bounding division.
    error=add(error,add(mul(u,add(1.,error)),(tiny/alpha).next_up()));
    error=add(error,add(mul(u,add(1.,error)),tiny));
    if model==Model::Cmyk {error=add(error,add(mul(u,add(2.,error)),tiny));}
    if !error.is_finite() || cancelled() {return None;}
    let mut out=[0.;4];out[..model.channels()].fill(error);Some(out)
}

#[test]
fn native_storage_envelope_covers_unit_colors_and_subnormal_alpha() {
    for model in [Model::Gray,Model::Rgb,Model::Cmyk] {
        for alpha in [f64::from_bits(1),f64::MIN_POSITIVE,1e-300,1e-12,0.6737398252860969,1.] {
            let bound=native_storage_error_bound(model,alpha,&mut 24,&||false).unwrap();
            for value in [0.,f64::from_bits(1),2f64.powi(-54),0.125,0.5,1.-f64::EPSILON/2.,1.] {
                let input=[value;4];let pixel=Pixel::from_native(model,&input[..model.channels()],alpha).unwrap();
                let recovered=pixel.native_components().unwrap();
                for i in 0..model.channels() {assert!((recovered[i]-value).abs()<=bound[i],"model={model:?} alpha={alpha} value={value}");}
                assert!(bound[model.channels()..].iter().all(|v|*v==0.));
            }
        }
    }
    for work in 0..24 {assert!(native_storage_error_bound(Model::Cmyk,1.,&mut {work},&||false).is_none());}
    for alpha in [0.,-1.,f64::NAN,f64::INFINITY,1.1] {assert!(native_storage_error_bound(Model::Cmyk,alpha,&mut 24,&||false).is_none());}
    let polls=std::cell::Cell::new(0);native_storage_error_bound(Model::Cmyk,1.,&mut 24,&||{polls.set(polls.get()+1);false}).unwrap();
    for stop in 1..=polls.get() {let n=std::cell::Cell::new(0);assert!(native_storage_error_bound(Model::Cmyk,1.,&mut 24,&||{n.set(n.get()+1);n.get()>=stop}).is_none());}
}

impl Pixel {
    pub(super) fn same_storage(self,other:Self)->bool {
        self.model==other.model && self.alpha.to_bits()==other.alpha.to_bits()
            && self.color.iter().zip(other.color).all(|(a,b)|a.to_bits()==b.to_bits())
    }

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
        // Source-over at full shape/opacity is exact replacement. Avoid an
        // unnecessary weighted backdrop sum that can perturb source bits.
        if source.alpha == 1. && shape == 1. && mode == BlendMode::Normal {
            return Some(source);
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

#[test]
fn normal_error_envelope_contains_native_component_perturbations_in_all_models() {
    for model in [Model::Gray,Model::Rgb,Model::Cmyk] {
        let n=model.channels();
        for alpha in [0.,0.125,0.5,1.] {for shape in [alpha,1.] {for background_alpha in [0.,0.25,1.] {
            for step in 0..64 {
                let c=step as f64/128.;let delta=1./256.;
                let components=[c;4];let shifted=[c+delta;4];
                let previous=Pixel::from_native(model,&components[..n],0.75).unwrap();
                let previous2=Pixel::from_native(model,&shifted[..n],0.75).unwrap();
                let backdrop=Pixel::from_native(model,&components[..n],background_alpha).unwrap();
                let backdrop2=Pixel::from_native(model,&shifted[..n],background_alpha).unwrap();
                let source=Pixel::from_native(model,&components[..n],alpha).unwrap();
                let source2=Pixel::from_native(model,&shifted[..n],alpha).unwrap();
                let a=Pixel::composite(previous,backdrop,source,shape,BlendMode::Normal).unwrap();
                let b=Pixel::composite(previous2,backdrop2,source2,shape,BlendMode::Normal).unwrap();
                let mut pe=[0.;4];let mut be=[0.;4];let mut se=[0.;4];
                pe[..n].fill(0.75*delta);be[..n].fill(background_alpha*delta);se[..n].fill(delta);
                let bound=normal_premultiplied_error_bound(model,pe,be,se,alpha,shape,&mut 100,&||false).unwrap();
                assert_eq!(a.alpha.to_bits(),b.alpha.to_bits());
                for channel in 0..n {assert!((a.color[channel]-b.color[channel]).abs()<=bound[channel]);}
            }
        }}}
    }
}
#[test]
fn normal_error_envelope_refuses_unsafe_inputs_work_and_every_cancel_checkpoint() {
    let z=[0.;4];let mut work=100;
    normal_premultiplied_error_bound(Model::Rgb,z,z,z,0.5,1.,&mut work,&||false).unwrap();
    for budget in 0..100-work {assert!(normal_premultiplied_error_bound(Model::Rgb,z,z,z,0.5,1.,&mut {budget},&||false).is_none());}
    for alpha in [f64::NAN,-1.,1.1,1e-300] {assert!(normal_premultiplied_error_bound(Model::Rgb,z,z,z,alpha,1.,&mut 100,&||false).is_none());}
    assert!(normal_premultiplied_error_bound(Model::Rgb,z,z,z,0.75,0.5,&mut 100,&||false).is_none());
    assert!(normal_premultiplied_error_bound(Model::Rgb,z,z,[0.,0.,0.,1.],0.5,1.,&mut 100,&||false).is_none());
    assert!(normal_premultiplied_error_bound(Model::Rgb,z,z,[f64::NAN,0.,0.,0.],0.5,1.,&mut 100,&||false).is_none());
    let polls=std::cell::Cell::new(0);
    normal_premultiplied_error_bound(Model::Rgb,z,z,z,0.5,1.,&mut 100,&||{polls.set(polls.get()+1);false}).unwrap();
    for stop in 1..=polls.get() {
        let count=std::cell::Cell::new(0);
        assert!(normal_premultiplied_error_bound(Model::Rgb,z,z,z,0.5,1.,&mut 100,&||{count.set(count.get()+1);count.get()>=stop}).is_none());
    }
}

#[test]
fn group_error_bounds_enclose_removal_opacity_and_native_conversion_in_all_models() {
    for model in [Model::Gray,Model::Rgb,Model::Cmyk] {
        let n=model.channels();
        for alpha in [0.,0.125,0.5,1.] {for background_alpha in [0.,0.5,1.] {for opacity in [0.,0.25,1.] {
            for i in 0..32 {
                let value=i as f64/128.;let delta=1./256.;let values=[value;4];let shifted=[value+delta;4];
                let initial=Pixel::from_native(model,&values[..n],background_alpha).unwrap();
                let initial2=Pixel::from_native(model,&shifted[..n],background_alpha).unwrap();
                let source=Pixel::from_native(model,&values[..n],alpha).unwrap();
                let source2=Pixel::from_native(model,&shifted[..n],alpha).unwrap();
                let a=Pixel::composite(initial,initial,source,1.,BlendMode::Normal).unwrap();
                let b=Pixel::composite(initial2,initial2,source2,1.,BlendMode::Normal).unwrap();
                let mut current=[0.;4];let mut backdrop=[0.;4];
                current[..n].fill((alpha+(1.-alpha)*background_alpha)*delta+64.*f64::EPSILON);
                backdrop[..n].fill(background_alpha*delta);
                let removed=group_error_bound(model,current,GroupErrorStep::RemoveBackdrop {alpha,initial:backdrop},&mut 100,&||false).unwrap();
                let a=a.remove_backdrop(initial,alpha).unwrap();let b=b.remove_backdrop(initial2,alpha).unwrap();
                for channel in 0..n {assert!((a.color[channel]-b.color[channel]).abs()<=removed[channel]);}
                let scaled=group_error_bound(model,removed,GroupErrorStep::ScaleOpacity {factor:opacity},&mut 100,&||false).unwrap();
                let a=a.scale_opacity(opacity).unwrap();let b=b.scale_opacity(opacity).unwrap();
                for channel in 0..n {assert!((a.color[channel]-b.color[channel]).abs()<=scaled[channel]);}
                let native=group_error_bound(model,scaled,GroupErrorStep::NativeComponents {alpha:a.alpha()},&mut 100,&||false).unwrap();
                let ac=a.native_components().unwrap();let bc=b.native_components().unwrap();
                for channel in 0..n {assert!((ac[channel]-bc[channel]).abs()<=native[channel]);}
            }
        }}}
    }
}
#[test]
fn group_error_bounds_refuse_invalid_storage_alpha_budgets_and_all_cancel_checkpoints() {
    let z=[0.;4];
    for step in [GroupErrorStep::RemoveBackdrop {alpha:0.5,initial:z},GroupErrorStep::ScaleOpacity {factor:0.5},GroupErrorStep::NativeComponents {alpha:0.5}] {
        let mut work=100;group_error_bound(Model::Rgb,z,step,&mut work,&||false).unwrap();
        for budget in 0..100-work {assert!(group_error_bound(Model::Rgb,z,step,&mut {budget},&||false).is_none());}
        let polls=std::cell::Cell::new(0);group_error_bound(Model::Rgb,z,step,&mut 100,&||{polls.set(polls.get()+1);false}).unwrap();
        for stop in 1..=polls.get() {let count=std::cell::Cell::new(0);assert!(group_error_bound(Model::Rgb,z,step,&mut 100,&||{count.set(count.get()+1);count.get()>=stop}).is_none());}
        assert!(group_error_bound(Model::Rgb,[f64::NAN,0.,0.,0.],step,&mut 100,&||false).is_none());
        assert!(group_error_bound(Model::Rgb,[0.,0.,0.,1.],step,&mut 100,&||false).is_none());
    }
    for step in [GroupErrorStep::RemoveBackdrop {alpha:-1.,initial:z},GroupErrorStep::RemoveBackdrop {alpha:0.5,initial:[-1.;4]},GroupErrorStep::ScaleOpacity {factor:1.1},GroupErrorStep::NativeComponents {alpha:1e-300}] {assert!(group_error_bound(Model::Rgb,z,step,&mut 100,&||false).is_none());}
}

#[test]
fn nonisolated_constant_hardlight_group_after_variable_parent_is_not_affine() {
    let evaluate=|component| {
        let parent=Pixel::from_native(Model::Gray,&[component],1.).unwrap();
        let constant=Pixel::from_native(Model::Gray,&[0.25],1.).unwrap();
        let child=Pixel::composite(parent,parent,constant,1.,BlendMode::HardLight).unwrap();
        let source=child.remove_backdrop(parent,1.).unwrap().scale_opacity(0.5).unwrap();
        Pixel::composite(parent,parent,source,1.,BlendMode::HardLight).unwrap().native_components().unwrap()[0]
    };
    assert_eq!(evaluate(0.),0.);assert_eq!(evaluate(1.),1.);
    assert_eq!(evaluate(0.5),0.375);
    assert_ne!((evaluate(0.)+evaluate(1.))/2.,evaluate(0.5));
}

#[test]
fn opaque_normal_source_replaces_backdrop_bit_exactly() {
    for model in [Model::Gray,Model::Rgb,Model::Cmyk] {
        for alpha in [0.,0.23,0.31,1./3.,0.77,1.] {
            let previous=Pixel::from_native(model,&[0.1,0.4,0.9,0.2][..model.channels()],alpha).unwrap();
            for value in [0.1f32,0.2,0.3,0.7,0.9] {
                let v=f64::from(value);
                let source=Pixel::from_native(model,&[v,1.-v,0.125,0.75][..model.channels()],1.).unwrap();
                let result=Pixel::composite(previous,previous,source,1.,BlendMode::Normal).unwrap();
                assert!(result.same_storage(source),"{model:?} alpha{alpha} source{v}: opaque paint must retain exact storage");
            }
        }
    }
}
