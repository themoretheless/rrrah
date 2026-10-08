//! PDF functions.
//!
//! PDF has the concept of functions, representing objects that take a certain number of values
//! as input, do some processing on them and then return some output.

mod type0;
mod type2;
mod type3;
mod type4;

use crate::function::type0::Type0;
use crate::function::type2::Type2;
use crate::function::type3::Type3;
use crate::function::type4::Type4;
use hayro_syntax::object::Dict;
use hayro_syntax::object::dict::keys::{DOMAIN, FUNCTION_TYPE, RANGE};
use hayro_syntax::object::{Object, dict_or_stream};
use smallvec::SmallVec;
use std::sync::Arc;

/// The input/output type of functions.
pub(crate) type Values = SmallVec<[f32; 4]>;
type TupleVec = SmallVec<[(f32, f32); 4]>;

#[derive(Debug)]
enum FunctionType {
    Type0(Type0),
    Type2(Type2),
    Type3(Type3),
    Type4(Type4),
}

/// A PDF function.
#[derive(Debug, Clone)]
pub struct Function(Arc<FunctionType>);

impl Function {
    /// Evaluate scalar PDF functions without heap scratch (linear sampled Order1).
    /// Work is limited to4096 steps and nesting to32 levels.
    pub fn eval_scalar_bounded(&self, input: f32, cancelled: &dyn Fn() -> bool) -> Option<f32> {
        self.eval_scalar_with_budget(input, cancelled, &mut 4096, 0)
    }

    pub(crate) fn eval_scalar_with_budget(&self, input: f32, cancelled: &dyn Fn() -> bool,
        remaining: &mut usize, depth: usize) -> Option<f32> {
        if cancelled() || depth >= 32 { return None; }
        *remaining = remaining.checked_sub(1)?;
        let result = match self.0.as_ref() {
            FunctionType::Type0(function) => function.eval_scalar(input, cancelled, remaining)?,
            FunctionType::Type2(function) => function.eval_scalar(input)?,
            FunctionType::Type3(function) => function.eval_scalar(input, cancelled, remaining, depth)?,
            FunctionType::Type4(function) => function.eval_scalar(input, cancelled, remaining, depth)?,
        };
        if cancelled() { return None; }
        Some(result)
    }
    /// Evaluate one input into up to four original color components without
    /// heap scratch or RGB conversion. Multi-output Type2/Type3 are supported;
    /// other types currently retain only the qualified scalar evaluator.
    /// Work is limited to4096 steps and nesting to32 levels. Failed or
    /// cancelled evaluation leaves the caller's output unchanged.
    pub fn eval_components_bounded(
        &self, input: f32, output: &mut [f32], cancelled: &dyn Fn() -> bool,
    ) -> Option<()> {
        if output.is_empty() || output.len() > 4 { return None; }
        let mut scratch = [0.0; 4];
        self.eval_components_with_budget(input, &mut scratch[..output.len()], cancelled, &mut 4096, 0)?;
        output.copy_from_slice(&scratch[..output.len()]);
        Some(())
    }

    pub(crate) fn eval_components_with_budget(
        &self, input: f32, output: &mut [f32], cancelled: &dyn Fn() -> bool,
        remaining: &mut usize, depth: usize,
    ) -> Option<()> {
        if output.is_empty() || output.len() > 4 || cancelled() || depth >= 32 { return None; }
        *remaining = remaining.checked_sub(1)?;
        match self.0.as_ref() {
            FunctionType::Type2(function) => {
                *remaining = remaining.checked_sub(output.len())?;
                function.eval_components(input, output, cancelled)?;
            }
            FunctionType::Type3(function) => function.eval_components(input, output, cancelled, remaining, depth)?,
            _ if output.len() == 1 => output[0] = self.eval_scalar_with_budget(input, cancelled, remaining, depth)?,
            _ => return None,
        }
        if cancelled() || output.iter().any(|value| !value.is_finite()) { return None; }
        Some(())
    }

    /// Conservative retained Arc/metadata capacity for native Type2/Type3
    /// source capture, bounded to4096 nodes and32 levels; no heap scratch.
    pub fn native_retained_capacity(&self, cancelled: &dyn Fn() -> bool) -> Option<usize> {
        self.native_retained_with_budget(cancelled, &mut 4096, 0)
    }
    pub(crate) fn native_retained_with_budget(&self, cancelled: &dyn Fn() -> bool,
        remaining: &mut usize, depth: usize) -> Option<usize> {
        if cancelled() || depth >= 32 { return None; }
        *remaining=remaining.checked_sub(1)?;
        let dynamic=match self.0.as_ref() {
            FunctionType::Type2(f)=>f.native_retained_capacity()?,
            FunctionType::Type3(f)=>f.native_retained_capacity(cancelled,remaining,depth)?,
            _=>return None,
        };
        if cancelled() { return None; }
        size_of::<FunctionType>().checked_add(2*size_of::<usize>())?.checked_add(dynamic)
    }

    /// Original PDF FunctionType number, without evaluating or converting it.
    pub fn source_type_number(&self) -> u8 {
        match self.0.as_ref() {FunctionType::Type0(_)=>0,FunctionType::Type2(_)=>2,FunctionType::Type3(_)=>3,FunctionType::Type4(_)=>4}
    }

    /// Create a new function.
    pub fn new(obj: &Object<'_>) -> Option<Self> {
        let (dict, stream) = dict_or_stream(obj)?;

        let function_type = match dict.get::<u8>(FUNCTION_TYPE)? {
            0 => FunctionType::Type0(Type0::new(stream?)?),
            2 => FunctionType::Type2(Type2::new(dict)?),
            3 => FunctionType::Type3(Type3::new(dict)?),
            4 => FunctionType::Type4(Type4::new(stream?)?),
            _ => return None,
        };

        Some(Self(Arc::new(function_type)))
    }

    /// Evaluate the function with the given input.
    pub fn eval(&self, input: Values) -> Option<Values> {
        match self.0.as_ref() {
            FunctionType::Type0(t0) => t0.eval(input),
            FunctionType::Type2(t2) => Some(t2.eval(*input.first()?)),
            FunctionType::Type3(t3) => t3.eval(*input.first()?),
            FunctionType::Type4(t4) => Some(t4.eval(input)?),
        }
    }
}

#[derive(Debug, Clone)]
struct Clamper {
    domain: TupleVec,
    range: Option<TupleVec>,
}

impl Clamper {
    fn native_retained_capacity(&self) -> Option<usize> {
        if self.domain.len()!=1 || self.range.as_ref().is_some_and(|r| r.is_empty() || r.len()>4) {return None;}
        let domain=if self.domain.spilled() {self.domain.capacity().checked_mul(size_of::<(f32,f32)>())?} else {0};
        let range=if let Some(r)=&self.range {if r.spilled() {r.capacity().checked_mul(size_of::<(f32,f32)>())?} else {0}} else {0};
        domain.checked_add(range)
    }
    fn new(dict: &Dict<'_>) -> Option<Self> {
        let domain = dict.get::<TupleVec>(DOMAIN)?;
        let range = dict.get::<TupleVec>(RANGE);

        Some(Self { domain, range })
    }

    fn clamp_input(&self, input: &mut [f32]) {
        if input.len() != self.domain.len() {
            warn!("the domain of the function didn't match the input arguments");
        }

        for ((min, max), val) in self.domain.iter().zip(input.iter_mut()) {
            *val = val.min(*max).max(*min);
        }
    }

    fn clamp_output(&self, output: &mut [f32]) {
        if let Some(range) = &self.range {
            if range.len() != output.len() {
                warn!("the range of the function didn't match the output arguments");
            }

            for ((min, max), val) in range.iter().zip(output.iter_mut()) {
                *val = val.min(*max).max(*min);
            }
        }
    }
}

/// Linearly interpolate the value `x`, assuming that it lies within the range `x_min` and `x_max`,
/// to the range `y_min` and `y_max`.
pub(crate) fn interpolate(x: f32, x_min: f32, x_max: f32, y_min: f32, y_max: f32) -> f32 {
    y_min + (x - x_min) * (y_max - y_min) / (x_max - x_min)
}
