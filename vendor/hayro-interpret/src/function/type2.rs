use crate::function::{Clamper, Values};
use hayro_syntax::object::Dict;
use hayro_syntax::object::Number;
use hayro_syntax::object::dict::keys::{C0, C1, N};
use smallvec::{SmallVec, smallvec};

/// A type 2 function (exponential function).
#[derive(Debug)]
pub(crate) struct Type2 {
    c0: Values,
    c1: Values,
    clamper: Clamper,
    n: f32,
}

impl Type2 {
    pub(crate) fn eval_scalar(&self, input: f32) -> Option<f32> {
        if !input.is_finite() || !self.n.is_finite()
            || self.c0.len() != 1 || self.c1.len() != 1 || self.clamper.domain.len() != 1
            || !self.c0[0].is_finite() || !self.c1[0].is_finite()
        { return None; }
        let (low, high) = self.clamper.domain[0];
        if !low.is_finite() || !high.is_finite() || low > high { return None; }
        let power = input.clamp(low, high).powf(self.n);
        let mut result = self.c0[0] + power * (self.c1[0] - self.c0[0]);
        if !power.is_finite() || !result.is_finite() { return None; }
        if let Some(range) = &self.clamper.range {
            if range.len() != 1 { return None; }
            let (low, high) = range[0];
            if !low.is_finite() || !high.is_finite() || low > high { return None; }
            result = result.clamp(low, high);
        }
        Some(result)
    }
    pub(crate) fn eval_components(&self, input: f32, output: &mut [f32],
        cancelled: &dyn Fn() -> bool) -> Option<()> {
        let count = output.len();
        if count == 0 || count > 4 || !input.is_finite() || !self.n.is_finite()
            || self.c0.len() != count || self.c1.len() != count || self.clamper.domain.len() != 1
            || self.clamper.range.as_ref().is_some_and(|range| range.len() != count)
        { return None; }
        let (low, high) = self.clamper.domain[0];
        if !low.is_finite() || !high.is_finite() || low > high { return None; }
        let power = input.clamp(low, high).powf(self.n);
        if !power.is_finite() { return None; }
        for (index, result) in output.iter_mut().enumerate() {
            if cancelled() { return None; }
            let (start, end) = (self.c0[index], self.c1[index]);
            if !start.is_finite() || !end.is_finite() { return None; }
            let mut value = start + power * (end - start);
            if !value.is_finite() { return None; }
            if let Some(range) = &self.clamper.range {
                let (low, high) = range[index];
                if !low.is_finite() || !high.is_finite() || low > high { return None; }
                value = value.clamp(low, high);
            }
            *result = value;
        }
        Some(())
    }

    pub(crate) fn native_retained_capacity(&self) -> Option<usize> {
        if self.c0.is_empty() || self.c0.len()>4 || self.c1.len()!=self.c0.len() {return None;}
        let mut bytes=self.clamper.native_retained_capacity()?;
        for values in [&self.c0,&self.c1] {
            if values.spilled() {bytes=bytes.checked_add(values.capacity().checked_mul(size_of::<f32>())?)?;}
        }
        Some(bytes)
    }
    /// Create a new type 2 function.
    pub(crate) fn new(dict: &Dict<'_>) -> Option<Self> {
        let c0 = dict.get::<Values>(C0).unwrap_or(smallvec![0.0]);
        let c1 = dict.get::<Values>(C1).unwrap_or(smallvec![1.0]);
        let clamper = Clamper::new(dict)?;
        let n = dict.get::<Number>(N)?.as_f64() as f32;

        Some(Self { c0, c1, clamper, n })
    }

    /// Evaluate the function with the given input.
    pub(crate) fn eval(&self, input: f32) -> Values {
        let mut input = [input];
        self.clamper.clamp_input(&mut input);

        let mut out = self
            .c0
            .iter()
            .zip(self.c1.iter())
            .map(|(c0, c1)| *c0 + input[0].powf(self.n) * (*c1 - *c0))
            .collect::<SmallVec<_>>();

        self.clamper.clamp_output(&mut out);

        out
    }
}

#[cfg(test)]
mod tests {
    use crate::function::Function;

    use hayro_syntax::object::Object;
    use hayro_syntax::object::{Dict, FromBytes};
    use smallvec::smallvec;

    #[test]
    fn native_rgb_cmyk_exponential_components_have_analytical_references() {
        let parse = |bytes: &[u8]| Function::new(&Object::Dict(Dict::from_bytes(bytes).unwrap())).unwrap();
        let rgb = parse(b"<< /FunctionType 2 /Domain [0 1] /C0 [0.125 0.25 0.5] /C1 [0.5 0.75 1] /N 1 >>");
        let mut values = [0.; 3];
        rgb.eval_components_bounded(0.5, &mut values, &|| false).unwrap();
        assert_eq!(values, [0.3125, 0.5, 0.75]);
        let cmyk = parse(b"<< /FunctionType 2 /Domain [0 1] /C0 [0 0.2 0.6 0.8] /C1 [1 0.6 0.2 0] /Range [0.1 0.2 0.2 0.4 0.3 0.7 0.4 0.9] /N 2 >>");
        for (input, expected) in [(-1., [0.1,0.2,0.6,0.8]), (0.5,[0.2,0.3,0.5,0.6]), (2.,[0.2,0.4,0.3,0.4])] {
            let mut output = [0.;4];
            cmyk.eval_components_bounded(input, &mut output, &|| false).unwrap();
            for (actual, expected) in output.into_iter().zip(expected) { assert!((actual-expected).abs() < 2e-7); }
        }
    }

    #[test]
    fn native_component_failure_and_every_cancellation_boundary_preserve_output() {
        let parse = |bytes: &[u8]| Function::new(&Object::Dict(Dict::from_bytes(bytes).unwrap())).unwrap();
        let valid = parse(b"<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [1 1 1 1] /N 1 >>");
        let polls = std::cell::Cell::new(0);
        valid.eval_components_bounded(0.5, &mut [0.;4], &|| { polls.set(polls.get()+1); false }).unwrap();
        let total = polls.get();
        assert_eq!(total, 6);
        for target in 1..=total {
            polls.set(0); let mut output = [9.;4];
            assert!(valid.eval_components_bounded(0.5, &mut output, &|| { polls.set(polls.get()+1); polls.get()==target }).is_none());
            assert_eq!(output,[9.;4]);
            assert_eq!(polls.get(),target);
        }
        for bytes in [
            &b"<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [1 1 1] /N 1 >>"[..],
            &b"<< /FunctionType 2 /Domain [1 0] /C0 [0 0 0 0] /C1 [1 1 1 1] /N 1 >>"[..],
            &b"<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [1 1 1 1] /Range [0 1 0 1 0 1 2 1] /N 1 >>"[..],
            &b"<< /FunctionType 2 /Domain [-1 1] /C0 [0 0 0 0] /C1 [1 1 1 1] /N 0.5 >>"[..],
        ] {
            let mut output=[9.;4];
            assert!(parse(bytes).eval_components_bounded(-0.5,&mut output,&|| false).is_none());
            assert_eq!(output,[9.;4]);
        }
        for input in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut output=[9.;4];
            assert!(valid.eval_components_bounded(input,&mut output,&|| false).is_none());
            assert_eq!(output,[9.;4]);
        }
        for count in [0usize, 1, 2, 3, 5] {
            let mut output=vec![9.;count];
            assert!(valid.eval_components_bounded(0.5,&mut output,&|| false).is_none());
            assert!(output.iter().all(|v| *v==9.));
        }
    }

    #[test]
    fn bounded_scalar_validates_domains_ranges_and_nonfinite_arithmetic() {
        let parse = |bytes: &[u8]| Function::new(&Object::Dict(Dict::from_bytes(bytes).unwrap())).unwrap();
        let function = parse(b"<< /FunctionType 2 /Domain [0.2 0.8] /Range [0.1 0.5] /C0 [0] /C1 [1] /N 2 >>");
        assert_eq!(function.eval_scalar_bounded(0.0, &|| false), Some(0.1));
        assert_eq!(function.eval_scalar_bounded(1.0, &|| false), Some(0.5));
        assert_eq!(function.eval_scalar_bounded(0.5, &|| false), Some(0.25));
        assert!(function.eval_scalar_bounded(f32::NAN, &|| false).is_none());
        let reversed = parse(b"<< /FunctionType 2 /Domain [1 0] /N 1 >>");
        assert!(reversed.eval_scalar_bounded(0.5, &|| false).is_none());
        let invalid_power = parse(b"<< /FunctionType 2 /Domain [-1 1] /N 0.5 >>");
        assert!(invalid_power.eval_scalar_bounded(-0.5, &|| false).is_none());
    }

    #[test]
    fn simple() {
        let func = Function::new(&Object::Dict(
            Dict::from_bytes(
                b"<<
              /FunctionType 2
              /Domain [ 0  1 ]
              /C0 [ 0 20  ]
              /C1 [ 30 -50 ]
              /N 1
            >>",
            )
            .unwrap(),
        ))
        .unwrap();

        assert_eq!(func.eval(smallvec![0.0]).unwrap().as_ref(), &[0.0, 20.0]);
        assert_eq!(func.eval(smallvec![0.5]).unwrap().as_ref(), &[15.0, -15.0]);
        assert_eq!(func.eval(smallvec![1.0]).unwrap().as_ref(), &[30.0, -50.0]);
    }

    #[test]
    fn with_exponent() {
        let func = Function::new(&Object::Dict(
            Dict::from_bytes(
                b"<<
              /FunctionType 2
              /Domain [ 0  1 ]
              /C0 [ 0  ]
              /C1 [ 30 ]
              /N 2
            >>",
            )
            .unwrap(),
        ))
        .unwrap();

        assert_eq!(func.eval(smallvec![0.5]), Some(smallvec![7.5]));
    }

    #[test]
    fn clamp_domain() {
        let func = Function::new(&Object::Dict(
            Dict::from_bytes(
                b"<<
              /FunctionType 2
              /Domain [ 0.2  0.8 ]
              /C0 [ 0  ]
              /C1 [ 30 ]
              /N 2
            >>",
            )
            .unwrap(),
        ))
        .unwrap();

        assert_eq!(
            func.eval(smallvec![0.0]).as_ref(),
            func.eval(smallvec![0.2]).as_ref(),
        );
        assert_eq!(
            func.eval(smallvec![-10.]).as_ref(),
            func.eval(smallvec![0.2]).as_ref(),
        );
        assert_eq!(
            func.eval(smallvec![0.8]).as_ref(),
            func.eval(smallvec![0.8]).as_ref(),
        );
        assert_eq!(
            func.eval(smallvec![1.2]).as_ref(),
            func.eval(smallvec![1.0]).as_ref(),
        );
    }

    #[test]
    fn clamp_range() {
        let func = Function::new(&Object::Dict(
            Dict::from_bytes(
                b"<<
              /FunctionType 2
              /Domain [ 0.0  1.0 ]
              /Range [10.0 20.0]
              /C0 [ 0  ]
              /C1 [ 30 ]
              /N 1
            >>",
            )
            .unwrap(),
        ))
        .unwrap();

        assert_eq!(func.eval(smallvec![0.0]), Some(smallvec![10.0]));
        assert_eq!(func.eval(smallvec![0.5]), Some(smallvec![15.0]));
        assert_eq!(func.eval(smallvec![1.0]), Some(smallvec![20.0]));
    }
}
