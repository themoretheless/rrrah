use crate::function::{Clamper, Function, TupleVec, Values, interpolate};
use hayro_syntax::object::Array;
use hayro_syntax::object::Dict;
use hayro_syntax::object::Object;
use hayro_syntax::object::dict::keys::{BOUNDS, ENCODE, FUNCTIONS};
use smallvec::smallvec;

/// A type 3 function (stitching function).
#[derive(Debug)]
pub(crate) struct Type3 {
    functions: Vec<Function>,
    bounds: Vec<f32>,
    encode: TupleVec,
    clamper: Clamper,
}

impl Type3 {
    pub(crate) fn eval_scalar(&self, input: f32, cancelled: &dyn Fn() -> bool,
        remaining: &mut usize, depth: usize) -> Option<f32> {
        let count = self.functions.len();
        if !input.is_finite() || count == 0 || self.clamper.domain.len() != 1
            || self.bounds.len() != count.checked_add(1)? || self.encode.len() != count
        { return None; }
        let (low, high) = self.clamper.domain[0];
        if !low.is_finite() || !high.is_finite() || low >= high { return None; }
        let mut previous = low;
        for &bound in &self.bounds[1..self.bounds.len()-1] {
            if cancelled() { return None; }
            *remaining = remaining.checked_sub(1)?;
            if !bound.is_finite() || bound <= previous || bound >= high { return None; }
            previous = bound;
        }
        let input = input.clamp(low, high);
        let index = self.bounds[1..self.bounds.len()-1].partition_point(|bound| *bound <= input);
        let start = if index == 0 { low } else { self.bounds[index] };
        let end = if index + 1 == count { high } else { self.bounds[index+1] };
        let (e0, e1) = self.encode[index];
        if !e0.is_finite() || !e1.is_finite() { return None; }
        // Use exact domain endpoints, not the legacy evaluator's dummy deltas.
        let encoded = interpolate(input, start, end, e0, e1);
        if !encoded.is_finite() { return None; }
        let mut output = self.functions[index].eval_scalar_with_budget(encoded, cancelled, remaining, depth+1)?;
        if let Some(range) = &self.clamper.range {
            if range.len() != 1 { return None; }
            let (low, high) = range[0];
            if !low.is_finite() || !high.is_finite() || low > high { return None; }
            output = output.clamp(low, high);
        }
        Some(output)
    }
    pub(crate) fn eval_components(&self, input: f32, output: &mut [f32],
        cancelled: &dyn Fn() -> bool, remaining: &mut usize, depth: usize) -> Option<()> {
        let count = self.functions.len();
        if !input.is_finite() || count == 0 || self.clamper.domain.len() != 1
            || self.bounds.len() != count.checked_add(1)? || self.encode.len() != count
            || self.clamper.range.as_ref().is_some_and(|range| range.len() != output.len())
        { return None; }
        let (low, high) = self.clamper.domain[0];
        if !low.is_finite() || !high.is_finite() || low >= high { return None; }
        let mut previous = low;
        for &bound in &self.bounds[1..self.bounds.len()-1] {
            if cancelled() { return None; }
            *remaining = remaining.checked_sub(1)?;
            if !bound.is_finite() || bound <= previous || bound >= high { return None; }
            previous = bound;
        }
        let input = input.clamp(low, high);
        let index = self.bounds[1..self.bounds.len()-1].partition_point(|bound| *bound <= input);
        let start = if index == 0 { low } else { self.bounds[index] };
        let end = if index + 1 == count { high } else { self.bounds[index+1] };
        let (e0, e1) = self.encode[index];
        if !e0.is_finite() || !e1.is_finite() { return None; }
        let encoded = interpolate(input, start, end, e0, e1);
        if !encoded.is_finite() { return None; }
        self.functions[index].eval_components_with_budget(encoded, output, cancelled, remaining, depth+1)?;
        if let Some(range) = &self.clamper.range {
            for (value, &(low, high)) in output.iter_mut().zip(range) {
                if cancelled() { return None; }
                *remaining = remaining.checked_sub(1)?;
                if !low.is_finite() || !high.is_finite() || low > high { return None; }
                *value = value.clamp(low, high);
            }
        }
        Some(())
    }

    pub(crate) fn native_retained_capacity(&self,cancelled:&dyn Fn()->bool,remaining:&mut usize,depth:usize)->Option<usize> {
        if self.functions.is_empty() || self.bounds.len()!=self.functions.len().checked_add(1)? || self.encode.len()!=self.functions.len() {return None;}
        let mut bytes=self.clamper.native_retained_capacity()?;
        bytes=bytes.checked_add(self.functions.capacity().checked_mul(size_of::<Function>())?)?;
        bytes=bytes.checked_add(self.bounds.capacity().checked_mul(size_of::<f32>())?)?;
        if self.encode.spilled() {bytes=bytes.checked_add(self.encode.capacity().checked_mul(size_of::<(f32,f32)>())?)?;}
        for f in &self.functions {bytes=bytes.checked_add(f.native_retained_with_budget(cancelled,remaining,depth+1)?)?;}
        Some(bytes)
    }
    /// Create a new type 3 function.
    pub(crate) fn new(dict: &Dict<'_>) -> Option<Self> {
        let clamper = Clamper::new(dict)?;

        let functions = dict
            .get::<Array<'_>>(FUNCTIONS)
            .and_then(|d| d.iter::<Object<'_>>().map(|o| Function::new(&o)).collect())?;
        let domain = *clamper.domain.first()?;
        let mut bounds = vec![domain.0 - 0.0001];
        if let Some(a) = dict.get::<Array<'_>>(BOUNDS) {
            bounds.extend(a.iter::<f32>());
        }
        // Add a small delta so that the interval is considered to be closed on the right.
        bounds.push(domain.1 + 0.0001);

        let encode = dict.get::<TupleVec>(ENCODE)?;

        Some(Self {
            functions,
            clamper,
            bounds,
            encode,
        })
    }

    /// Evaluate the function with the given input.
    pub(crate) fn eval(&self, input: f32) -> Option<Values> {
        let mut input = [input];
        self.clamper.clamp_input(&mut input);

        let index = find_interval(&self.bounds, input[0])?;

        let bounds_i = *self.bounds.get(index + 1)?;
        let bounds_i_minus_1 = *self.bounds.get(index)?;

        // - 1 because we inserted a dummy bound in the constructor.
        let encoding = self.encode.get(index)?;
        let function = self.functions.get(index)?;
        let encoded = interpolate(input[0], bounds_i_minus_1, bounds_i, encoding.0, encoding.1);

        let mut evaluated = function.eval(smallvec![encoded])?;

        self.clamper.clamp_output(&mut evaluated);

        Some(evaluated)
    }
}

fn find_interval(bounds: &[f32], x: f32) -> Option<usize> {
    if x < *bounds.first()? || x >= *bounds.last()? {
        return None;
    }

    match bounds.binary_search_by(|val| {
        if *val <= x {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    }) {
        Ok(i) => Some(i - 1),
        Err(i) => Some(i - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayro_syntax::object::FromBytes;

    fn native_cmyk_stitch() -> Function {
        Function::new(&Object::from_bytes(b"<< /FunctionType 3 /Domain [0 1] /Functions [ << /FunctionType 2 /Domain [0 1] /C0 [0 0.25 0.5 0.75] /C1 [1 0.75 0 0.25] /N 1 >> << /FunctionType 2 /Domain [0 1] /C0 [0.125 0.875 0.25 0.625] /C1 [0.625 0.375 0.75 0.125] /N 1 >> ] /Bounds [0.5] /Encode [0 1 0 1] >>").unwrap()).unwrap()
    }

    #[test]
    fn native_cmyk_stitch_has_exact_endpoints_and_right_interval_reference() {
        let function = native_cmyk_stitch();
        for (input, expected) in [
            (-1.,[0.,0.25,0.5,0.75]), (0.,[0.,0.25,0.5,0.75]),
            (0.25,[0.5,0.5,0.25,0.5]), (0.5,[0.125,0.875,0.25,0.625]),
            (0.75,[0.375,0.625,0.5,0.375]), (1.,[0.625,0.375,0.75,0.125]),
            (2.,[0.625,0.375,0.75,0.125]),
        ] {
            let mut output=[9.;4];
            function.eval_components_bounded(input,&mut output,&|| false).unwrap();
            assert_eq!(output,expected);
        }
    }

    #[test]
    fn native_stitch_cancellation_and_shared_work_depth_bounds_preserve_output() {
        let function=native_cmyk_stitch();
        let polls=std::cell::Cell::new(0);
        function.eval_components_bounded(0.25,&mut [0.;4],&|| { polls.set(polls.get()+1);false }).unwrap();
        let total=polls.get();
        assert_eq!(total,9);
        for target in 1..=total {
            polls.set(0);let mut output=[9.;4];
            assert!(function.eval_components_bounded(0.25,&mut output,&|| { polls.set(polls.get()+1);polls.get()==target }).is_none());
            assert_eq!(output,[9.;4]);assert_eq!(polls.get(),target);
        }
        assert!(function.eval_components_with_budget(0.25,&mut [0.;4],&|| false,&mut 6,0).is_none());
        assert!(function.eval_components_with_budget(0.25,&mut [0.;4],&|| false,&mut 7,0).is_some());
        let leaf="<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [1 1 1 1] /N 1 >>";
        for nesting in [31,32] {
            let mut data=leaf.to_owned();
            for _ in 0..nesting { data=format!("<< /FunctionType 3 /Domain [0 1] /Functions [{data}] /Bounds [] /Encode [0 1] >>"); }
            let nested=Function::new(&Object::from_bytes(data.as_bytes()).unwrap()).unwrap();
            let mut output=[9.;4];
            let result=nested.eval_components_bounded(0.5,&mut output,&|| false);
            if nesting==31 { assert!(result.is_some());assert_eq!(output,[0.5;4]); }
            else { assert!(result.is_none());assert_eq!(output,[9.;4]); }
        }
    }

    #[test]
    fn native_stitch_refuses_malformed_ranges_bounds_and_widths_without_publication() {
        for bytes in [
            &b"<< /FunctionType 3 /Domain [0 1] /Functions [ << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [1 1 1 1] /N 1 >> << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [1 1 1 1] /N 1 >> ] /Bounds [1.5] /Encode [0 1 0 1] >>"[..],
            &b"<< /FunctionType 3 /Domain [0 1] /Functions [ << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [1 1 1 1] /N 1 >> ] /Bounds [] /Encode [0 1] /Range [0 1 0 1 0 1 2 1] >>"[..],
            &b"<< /FunctionType 3 /Domain [0 1] /Functions [ << /FunctionType 2 /Domain [0 1] /N 1 >> ] /Bounds [] /Encode [0 1] >>"[..],
        ] {
            let function=Function::new(&Object::from_bytes(bytes).unwrap()).unwrap();
            let mut output=[9.;4];assert!(function.eval_components_bounded(0.5,&mut output,&|| false).is_none());
            assert_eq!(output,[9.;4]);
        }
    }

    #[test]
    fn bounded_scalar_uses_exact_endpoints_and_right_interval_at_boundary() {
        let function = Function::new(&Object::from_bytes(b"<< /FunctionType 3 /Domain [0 1] /Functions [ << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >> << /FunctionType 2 /Domain [0 1] /C0 [0.2] /C1 [0.4] /N 1 >> ] /Bounds [0.5] /Encode [0 1 0 1] >>").unwrap()).unwrap();
        for (input, expected) in [(0.0,0.0),(0.25,0.5),(0.5,0.2),(0.75,0.3),(1.0,0.4),(-1.0,0.0),(2.0,0.4)] {
            assert!((function.eval_scalar_bounded(input, &|| false).unwrap()-expected).abs() < 1e-7);
        }
        for stop in 1..=5 {
            let polls = std::cell::Cell::new(0);
            assert!(function.eval_scalar_bounded(0.25, &|| {
                polls.set(polls.get()+1); polls.get()==stop
            }).is_none());
        }
        assert!(function.eval_scalar_with_budget(0.25, &|| false, &mut 1, 0).is_none());
        assert!(function.eval_scalar_bounded(f32::NAN, &|| false).is_none());
    }

    #[test]
    fn bounded_scalar_rejects_deep_nesting_and_invalid_bounds() {
        let mut data = "<< /FunctionType 2 /Domain [0 1] /N 1 >>".to_owned();
        for _ in 0..32 {
            data = format!("<< /FunctionType 3 /Domain [0 1] /Functions [{data}] /Bounds [] /Encode [0 1] >>");
        }
        let function = Function::new(&Object::from_bytes(data.as_bytes()).unwrap()).unwrap();
        assert!(function.eval_scalar_bounded(0.5, &|| false).is_none());
        let invalid = Function::new(&Object::from_bytes(b"<< /FunctionType 3 /Domain [0 1] /Functions [ << /FunctionType 2 /Domain [0 1] /N 1 >> << /FunctionType 2 /Domain [0 1] /N 1 >> ] /Bounds [1.5] /Encode [0 1 0 1] >>").unwrap()).unwrap();
        assert!(invalid.eval_scalar_bounded(0.5, &|| false).is_none());
    }

    #[test]
    fn simple() {
        let data = b"<<
  /FunctionType 3
  /Domain [-7 7]
  /Functions [
    << /FunctionType 2
       /Domain [0 1]
       /C0 [0.5 0.5 0.5]
       /C1 [0.5 0.5 0.5]
       /N 1
    >>
    << /FunctionType 2
       /Domain [0 1]
       /C0 [0.7 0.7 0.7]
       /C1 [0.7 0.7 0.7]
       /N 1
    >>
  ]
  /Bounds [0]
  /Encode [0 1 0 1]
>>";

        let dict = Object::from_bytes(data).unwrap();
        let function = Function::new(&dict).unwrap();

        assert_eq!(
            function.eval(smallvec![-7.0]).unwrap().as_slice(),
            &[0.5, 0.5, 0.5]
        );
        assert_eq!(
            function.eval(smallvec![-3.0]).unwrap().as_slice(),
            &[0.5, 0.5, 0.5]
        );
        assert_eq!(
            function.eval(smallvec![-0.5]).unwrap().as_slice(),
            &[0.5, 0.5, 0.5]
        );
        assert_eq!(
            function.eval(smallvec![0.0]).unwrap().as_slice(),
            &[0.7, 0.7, 0.7]
        );
        assert_eq!(
            function.eval(smallvec![7.0]).unwrap().as_slice(),
            &[0.7, 0.7, 0.7]
        );
    }
}
