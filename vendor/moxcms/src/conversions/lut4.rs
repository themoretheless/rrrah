/*
 * // Copyright (c) Radzivon Bartoshyk 3/2025. All rights reserved.
 * //
 * // Redistribution and use in source and binary forms, with or without modification,
 * // are permitted provided that the following conditions are met:
 * //
 * // 1.  Redistributions of source code must retain the above copyright notice, this
 * // list of conditions and the following disclaimer.
 * //
 * // 2.  Redistributions in binary form must reproduce the above copyright notice,
 * // this list of conditions and the following disclaimer in the documentation
 * // and/or other materials provided with the distribution.
 * //
 * // 3.  Neither the name of the copyright holder nor the names of its
 * // contributors may be used to endorse or promote products derived from
 * // this software without specific prior written permission.
 * //
 * // THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * // AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * // IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * // DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 * // FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * // DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * // SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 * // CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 * // OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * // OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
#![cfg(feature = "lut")]
#[cfg(feature = "any_to_any")]
use crate::conversions::katana::KatanaInitialStage;
use crate::err::try_vec;
use crate::profile::LutDataType;
use crate::safe_math::{SafeMul, SafePowi};
use crate::trc::lut_interp_linear_float;
use crate::{
    CmsError, DataColorSpace, Hypercube, InterpolationMethod, MalformedSize,
    PointeeSizeExpressible, Stage, TransformOptions, Vector3f,
};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

#[allow(unused)]
#[derive(Default)]
struct Lut4x3 {
    linearization: [Vec<f32>; 4],
    clut: Vec<f32>,
    grid_size: u8,
    output: [Vec<f32>; 3],
    interpolation_method: InterpolationMethod,
    pcs: DataColorSpace,
}

#[allow(unused)]
#[derive(Default)]
struct KatanaLut4x3<T: Copy + PointeeSizeExpressible + AsPrimitive<f32>> {
    linearization: [Vec<f32>; 4],
    clut: Vec<f32>,
    grid_size: u8,
    output: [Vec<f32>; 3],
    interpolation_method: InterpolationMethod,
    pcs: DataColorSpace,
    _phantom: PhantomData<T>,
    bit_depth: usize,
    rrrah_cmyk_hybrid: bool,
}

#[allow(unused)]
impl Lut4x3 {
    fn transform_impl<Fetch: Fn(f32, f32, f32, f32) -> Vector3f>(
        &self,
        src: &[f32],
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        let linearization_0 = &self.linearization[0];
        let linearization_1 = &self.linearization[1];
        let linearization_2 = &self.linearization[2];
        let linearization_3 = &self.linearization[3];
        for (dest, src) in dst.chunks_exact_mut(3).zip(src.chunks_exact(4)) {
            debug_assert!(self.grid_size as i32 >= 1);
            let linear_x = lut_interp_linear_float(src[0], linearization_0);
            let linear_y = lut_interp_linear_float(src[1], linearization_1);
            let linear_z = lut_interp_linear_float(src[2], linearization_2);
            let linear_w = lut_interp_linear_float(src[3], linearization_3);

            let clut = fetch(linear_x, linear_y, linear_z, linear_w);

            let pcs_x = lut_interp_linear_float(clut.v[0], &self.output[0]);
            let pcs_y = lut_interp_linear_float(clut.v[1], &self.output[1]);
            let pcs_z = lut_interp_linear_float(clut.v[2], &self.output[2]);
            dest[0] = pcs_x;
            dest[1] = pcs_y;
            dest[2] = pcs_z;
        }
        Ok(())
    }
}

macro_rules! define_lut4_dispatch {
    ($dispatcher: ident) => {
        impl Stage for $dispatcher {
            fn transform(&self, src: &[f32], dst: &mut [f32]) -> Result<(), CmsError> {
                let l_tbl = Hypercube::new(&self.clut, self.grid_size as usize, 3)?;

                // If Source PCS is LAB trilinear should be used
                if self.pcs == DataColorSpace::Lab || self.pcs == DataColorSpace::Xyz {
                    return self
                        .transform_impl(src, dst, |x, y, z, w| l_tbl.quadlinear_vec3(x, y, z, w));
                }

                match self.interpolation_method {
                    #[cfg(feature = "options")]
                    InterpolationMethod::Tetrahedral => {
                        self.transform_impl(src, dst, |x, y, z, w| l_tbl.tetra_vec3(x, y, z, w))?;
                    }
                    #[cfg(feature = "options")]
                    InterpolationMethod::Pyramid => {
                        self.transform_impl(src, dst, |x, y, z, w| l_tbl.pyramid_vec3(x, y, z, w))?;
                    }
                    #[cfg(feature = "options")]
                    InterpolationMethod::Prism => {
                        self.transform_impl(src, dst, |x, y, z, w| l_tbl.prism_vec3(x, y, z, w))?
                    }
                    InterpolationMethod::Linear => {
                        self.transform_impl(src, dst, |x, y, z, w| {
                            l_tbl.quadlinear_vec3(x, y, z, w)
                        })?
                    }
                }
                Ok(())
            }
        }
    };
}

#[cfg(feature = "any_to_any")]
impl<T: Copy + PointeeSizeExpressible + AsPrimitive<f32>> KatanaLut4x3<T> {
    fn hybrid_fetch(&self, c: f32, m: f32, y: f32, k: f32) -> Vector3f {
        let scale = (self.grid_size - 1) as f32;
        let coordinates = [c, m, y, k].map(|v| v.clamp(0.0, 1.0) * scale);
        let lower = coordinates.map(|v| (v.floor() as usize).min(self.grid_size as usize - 2));
        let fractions = std::array::from_fn::<_, 4, _>(|i| coordinates[i] - lower[i] as f32);
        let mut order = [1usize, 2, 3];
        // Stable sorting network for exactly three tetrahedral coordinates.
        if fractions[order[0]] < fractions[order[1]] { order.swap(0, 1); }
        if fractions[order[1]] < fractions[order[2]] { order.swap(1, 2); }
        if fractions[order[0]] < fractions[order[1]] { order.swap(0, 1); }
        let weights = [1.0-fractions[order[0]], fractions[order[0]]-fractions[order[1]], fractions[order[1]]-fractions[order[2]], fractions[order[2]]];
        let n = self.grid_size as usize;
        let mut result = [0.0; 3];
        for side in 0..2 {
            let mut point = lower;
            point[0] += side;
            let outer = if side == 0 { 1.0-fractions[0] } else { fractions[0] };
            for step in 0..4 {
                let base = (((point[0]*n+point[1])*n+point[2])*n+point[3])*3;
                for channel in 0..3 { result[channel] += outer*weights[step]*self.clut[base+channel]; }
                if step < 3 { point[order[step]] += 1; }
            }
        }
        Vector3f { v: result }
    }

    // Bounds the same hybrid CLUT arithmetic over a closed input box. Curves
    // before/after this stage and PCS conversion remain separate obligations.
    #[allow(dead_code)]
    fn hybrid_fetch_bounds(&self,input:[[f32;2];4],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[[f32;2];3]> {
        hybrid_clut_bounds(&self.clut,usize::from(self.grid_size),input,work,cancelled)
    }

    #[allow(dead_code)]
    fn hybrid_to_pcs_bounds(&self,input:[[f32;2];4],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[[f32;2];3]> {
        if !self.rrrah_cmyk_hybrid || cancelled() {return None;}
        let mut linear=[[0.;2];4];
        for axis in 0..4 {
            linear[axis]=hybrid_curve_bounds(&self.linearization[axis],input[axis],work,cancelled)?;
            // The point CLUT clamps the result of each input curve.
            linear[axis]=[linear[axis][0].clamp(0.,1.),linear[axis][1].clamp(0.,1.)];
        }
        let clut=self.hybrid_fetch_bounds(linear,work,cancelled)?;
        let mut output=[[0.;2];3];
        for channel in 0..3 {output[channel]=hybrid_curve_bounds(&self.output[channel],clut[channel],work,cancelled)?;}
        if cancelled() {return None;}Some(output)
    }

    fn to_pcs_impl<Fetch: Fn(f32, f32, f32, f32) -> Vector3f>(
        &self,
        input: &[T],
        fetch: Fetch,
    ) -> Result<Vec<f32>, CmsError> {
        if input.len() % 4 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        let norm_value = if T::FINITE {
            1.0 / ((1u32 << self.bit_depth) - 1) as f32
        } else {
            1.0
        };
        let mut dst = try_vec![0.; (input.len() / 4) * 3];
        let linearization_0 = &self.linearization[0];
        let linearization_1 = &self.linearization[1];
        let linearization_2 = &self.linearization[2];
        let linearization_3 = &self.linearization[3];
        for (dest, src) in dst.chunks_exact_mut(3).zip(input.chunks_exact(4)) {
            let linear_x = lut_interp_linear_float(src[0].as_() * norm_value, linearization_0);
            let linear_y = lut_interp_linear_float(src[1].as_() * norm_value, linearization_1);
            let linear_z = lut_interp_linear_float(src[2].as_() * norm_value, linearization_2);
            let linear_w = lut_interp_linear_float(src[3].as_() * norm_value, linearization_3);

            let clut = fetch(linear_x, linear_y, linear_z, linear_w);

            let pcs_x = lut_interp_linear_float(clut.v[0], &self.output[0]);
            let pcs_y = lut_interp_linear_float(clut.v[1], &self.output[1]);
            let pcs_z = lut_interp_linear_float(clut.v[2], &self.output[2]);
            dest[0] = pcs_x;
            dest[1] = pcs_y;
            dest[2] = pcs_z;
        }
        Ok(dst)
    }
}

#[cfg(feature = "any_to_any")]
impl<T: Copy + PointeeSizeExpressible + AsPrimitive<f32>> KatanaInitialStage<f32, T>
    for KatanaLut4x3<T>
{
    fn bound_float_cmyk(&self,input:[[f32;2];4],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[[f32;2];3]> {
        if T::FINITE || std::mem::size_of::<T>()!=4 {return None;}
        self.hybrid_to_pcs_bounds(input,work,cancelled)
    }
    fn to_pcs(&self, input: &[T]) -> Result<Vec<f32>, CmsError> {
        if input.len() % 4 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        if self.rrrah_cmyk_hybrid {
            return self.to_pcs_impl(input, |c, m, y, k| self.hybrid_fetch(c, m, y, k));
        }
        let l_tbl = Hypercube::new(&self.clut, self.grid_size as usize, 3)?;

        // If Source PCS is LAB trilinear should be used
        if self.pcs == DataColorSpace::Lab || self.pcs == DataColorSpace::Xyz {
            return self.to_pcs_impl(input, |x, y, z, w| l_tbl.quadlinear_vec3(x, y, z, w));
        }

        match self.interpolation_method {
            #[cfg(feature = "options")]
            InterpolationMethod::Tetrahedral => {
                self.to_pcs_impl(input, |x, y, z, w| l_tbl.tetra_vec3(x, y, z, w))
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Pyramid => {
                self.to_pcs_impl(input, |x, y, z, w| l_tbl.pyramid_vec3(x, y, z, w))
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Prism => {
                self.to_pcs_impl(input, |x, y, z, w| l_tbl.prism_vec3(x, y, z, w))
            }
            InterpolationMethod::Linear => {
                self.to_pcs_impl(input, |x, y, z, w| l_tbl.quadlinear_vec3(x, y, z, w))
            }
        }
    }
}

define_lut4_dispatch!(Lut4x3);

fn make_lut_4x3(
    lut: &LutDataType,
    options: TransformOptions,
    pcs: DataColorSpace,
) -> Result<Lut4x3, CmsError> {
    // There is 4 possible cases:
    // - All curves are non-linear
    // - Linearization curves are non-linear, but gamma is linear
    // - Gamma curves are non-linear, but linearization is linear
    // - All curves linear
    let clut_length: usize = (lut.num_clut_grid_points as usize)
        .safe_powi(lut.num_input_channels as u32)?
        .safe_mul(lut.num_output_channels as usize)?;

    let clut_table = lut.clut_table.to_clut_f32();
    if clut_table.len() != clut_length {
        return Err(CmsError::MalformedClut(MalformedSize {
            size: clut_table.len(),
            expected: clut_length,
        }));
    }

    let linearization_table = lut.input_table.to_clut_f32();

    if linearization_table.len() < lut.num_input_table_entries as usize * 4 {
        return Err(CmsError::MalformedCurveLutTable(MalformedSize {
            size: linearization_table.len(),
            expected: lut.num_input_table_entries as usize * 4,
        }));
    }

    let lin_curve0 = linearization_table[0..lut.num_input_table_entries as usize].to_vec();
    let lin_curve1 = linearization_table
        [lut.num_input_table_entries as usize..lut.num_input_table_entries as usize * 2]
        .to_vec();
    let lin_curve2 = linearization_table
        [lut.num_input_table_entries as usize * 2..lut.num_input_table_entries as usize * 3]
        .to_vec();
    let lin_curve3 = linearization_table
        [lut.num_input_table_entries as usize * 3..lut.num_input_table_entries as usize * 4]
        .to_vec();

    let gamma_table = lut.output_table.to_clut_f32();

    if gamma_table.len() < lut.num_output_table_entries as usize * 3 {
        return Err(CmsError::MalformedCurveLutTable(MalformedSize {
            size: gamma_table.len(),
            expected: lut.num_output_table_entries as usize * 3,
        }));
    }

    let gamma_curve0 = gamma_table[..lut.num_output_table_entries as usize].to_vec();
    let gamma_curve1 = gamma_table
        [lut.num_output_table_entries as usize..lut.num_output_table_entries as usize * 2]
        .to_vec();
    let gamma_curve2 = gamma_table
        [lut.num_output_table_entries as usize * 2..lut.num_output_table_entries as usize * 3]
        .to_vec();

    let transform = Lut4x3 {
        linearization: [lin_curve0, lin_curve1, lin_curve2, lin_curve3],
        interpolation_method: options.interpolation_method,
        pcs,
        clut: clut_table,
        grid_size: lut.num_clut_grid_points,
        output: [gamma_curve0, gamma_curve1, gamma_curve2],
    };
    Ok(transform)
}

fn stage_lut_4x3(
    lut: &LutDataType,
    options: TransformOptions,
    pcs: DataColorSpace,
) -> Result<Box<dyn Stage>, CmsError> {
    let lut = make_lut_4x3(lut, options, pcs)?;
    let transform = Lut4x3 {
        linearization: lut.linearization,
        interpolation_method: lut.interpolation_method,
        pcs: lut.pcs,
        clut: lut.clut,
        grid_size: lut.grid_size,
        output: lut.output,
    };
    Ok(Box::new(transform))
}

#[cfg(feature = "any_to_any")]
pub(crate) fn katana_input_stage_lut_4x3<
    T: Copy + PointeeSizeExpressible + AsPrimitive<f32> + Send + Sync,
>(
    lut: &LutDataType,
    options: TransformOptions,
    pcs: DataColorSpace,
    bit_depth: usize,
) -> Result<Box<dyn KatanaInitialStage<f32, T> + Send + Sync>, CmsError> {
    // There is 4 possible cases:
    // - All curves are non-linear
    // - Linearization curves are non-linear, but gamma is linear
    // - Gamma curves are non-linear, but linearization is linear
    // - All curves linear
    let lut = make_lut_4x3(lut, options, pcs)?;

    let transform = KatanaLut4x3::<T> {
        linearization: lut.linearization,
        interpolation_method: lut.interpolation_method,
        pcs: lut.pcs,
        clut: lut.clut,
        grid_size: lut.grid_size,
        output: lut.output,
        _phantom: PhantomData,
        bit_depth,
        rrrah_cmyk_hybrid: options.rrrah_cmyk_hybrid,
    };
    Ok(Box::new(transform))
}

pub(crate) fn create_lut4_norm_samples<const SAMPLES: usize>() -> Vec<f32> {
    let lut_size: u32 = (4 * SAMPLES * SAMPLES * SAMPLES * SAMPLES) as u32;

    let mut src = Vec::with_capacity(lut_size as usize);

    let recpeq = 1f32 / (SAMPLES - 1) as f32;
    for k in 0..SAMPLES {
        for c in 0..SAMPLES {
            for m in 0..SAMPLES {
                for y in 0..SAMPLES {
                    src.push(c as f32 * recpeq);
                    src.push(m as f32 * recpeq);
                    src.push(y as f32 * recpeq);
                    src.push(k as f32 * recpeq);
                }
            }
        }
    }
    src
}

pub(crate) fn create_lut4<const SAMPLES: usize>(
    lut: &LutDataType,
    options: TransformOptions,
    pcs: DataColorSpace,
) -> Result<Vec<f32>, CmsError> {
    if lut.num_input_channels != 4 {
        return Err(CmsError::UnsupportedProfileConnection);
    }
    let lut_size: u32 = (4 * SAMPLES * SAMPLES * SAMPLES * SAMPLES) as u32;

    let src = create_lut4_norm_samples::<SAMPLES>();
    let mut dest = try_vec![0.; (lut_size as usize) / 4 * 3];

    let lut_stage = stage_lut_4x3(lut, options, pcs)?;
    lut_stage.transform(&src, &mut dest)?;
    Ok(dest)
}

// Explicit adjacent-float operations preserve the vendored crate's Rust1.85
// minimum; std f32 next_up/next_down stabilized after that version.
pub(super) fn hybrid_next_up(v:f32)->f32 {
    if v.is_nan() || v==f32::INFINITY {return v;}if v==0. {return f32::from_bits(1);}
    f32::from_bits(if v>0. {v.to_bits()+1} else {v.to_bits()-1})
}
pub(super) fn hybrid_next_down(v:f32)->f32 {
    if v.is_nan() || v==f32::NEG_INFINITY {return v;}if v==0. {return -f32::from_bits(1);}
    f32::from_bits(if v>0. {v.to_bits()-1} else {v.to_bits()+1})
}
// Outward f32 intervals enclose both separate and fused evaluation because
// every intermediate interval also encloses its exact real arithmetic result.
#[derive(Clone,Copy)]
pub(super) struct HybridInterval(pub(super) [f32;2]);
impl HybridInterval {
    pub(super) fn new(v:[f32;2])->Option<Self> {(v[0].is_finite() && v[1].is_finite() && v[0]<=v[1]).then_some(Self(v))}
    pub(super) fn point(v:f32)->Option<Self> {Self::new([v,v])}
    pub(super) fn op(self,b:Self,kind:u8,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<Self> {
        if cancelled() {return None;}*work=work.checked_sub(1)?;
        let [a0,a1]=self.0;let [b0,b1]=b.0;
        let [lo,hi]=match kind {
            0=>[a0+b0,a1+b1],1=>[a0-b1,a1-b0],
            2=>{let v=[a0*b0,a0*b1,a1*b0,a1*b1];[v.into_iter().fold(f32::INFINITY,f32::min),v.into_iter().fold(f32::NEG_INFINITY,f32::max)]},
            3=>{if b0<=0. && b1>=0. {return None;}let v=[a0/b0,a0/b1,a1/b0,a1/b1];[v.into_iter().fold(f32::INFINITY,f32::min),v.into_iter().fold(f32::NEG_INFINITY,f32::max)]},_=>return None,
        };
        Self::new([hybrid_next_down(lo),hybrid_next_up(hi)])
    }
}
// Bound the actual curve arithmetic, retaining its shared diff correlation.
// Reassociation is used only for the enclosure, never for point execution:
// tu - tu*diff + tl*diff = tu + (tl-tu)*diff in exact arithmetic. The original
// rounded path differs by at most8*EPSILON*M+8*minsub (four mul/add stages,
// |tu|,|tl|<=M, diff in[0,1]); this also covers fused versions of that path.
#[allow(dead_code)]
fn hybrid_curve_bounds(table:&[f32],input:[f32;2],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[f32;2]> {
    if cancelled() {return None;}*work=work.checked_sub(6)?;
    let input=HybridInterval::new(input)?;
    if table.is_empty() || table.len()>65536 {return None;}
    if table.len()==1 {return Some(HybridInterval::point(table[0])?.0);}
    let scale=(table.len()-1) as f32;
    let coordinate=HybridInterval::new([input.0[0].clamp(0.,1.),input.0[1].clamp(0.,1.)])?
        .op(HybridInterval::point(scale)?,2,work,cancelled)?;
    let coordinate=HybridInterval::new([coordinate.0[0].max(0.),coordinate.0[1].min(scale)])?;
    let first=(coordinate.0[0].floor() as usize).min(table.len()-2);
    let last=(coordinate.0[1].floor() as usize).min(table.len()-2);
    let mut result=[f32::INFINITY,f32::NEG_INFINITY];
    for index in first..=last {
        if cancelled() {return None;}*work=work.checked_sub(4)?;
        let value=HybridInterval::new([coordinate.0[0].max(index as f32),coordinate.0[1].min((index+1) as f32)])?;
        let diff=HybridInterval::point((index+1) as f32)?.op(value,1,work,cancelled)?;
        let diff=HybridInterval::new([diff.0[0].max(0.),diff.0[1].min(1.)])?;
        let lo=HybridInterval::point(table[index])?;let hi=HybridInterval::point(table[index+1])?;
        let exact=lo.op(hi,1,work,cancelled)?.op(diff,2,work,cancelled)?.op(hi,0,work,cancelled)?;
        let magnitude=table[index].abs().max(table[index+1].abs());
        let error=hybrid_next_up((8.*f64::from(f32::EPSILON)*f64::from(magnitude)+8.*f64::from(f32::from_bits(1))) as f32);
        if !error.is_finite() {return None;}
        let error=HybridInterval::point(error)?;
        let lower=exact.op(error,1,work,cancelled)?.0[0];
        let upper=exact.op(error,0,work,cancelled)?.0[1];
        result[0]=result[0].min(lower);result[1]=result[1].max(upper);
    }
    if cancelled() {return None;}HybridInterval::new(result).map(|v|v.0)
}

#[allow(dead_code)]
fn hybrid_clut_bounds(clut:&[f32],n:usize,input:[[f32;2];4],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[[f32;2];3]> {
    if cancelled() {return None;}*work=work.checked_sub(16)?;
    if !(2..=255).contains(&n) || clut.len()!=n.checked_pow(4)?.checked_mul(3)? {return None;}
    let mut fractions=[HybridInterval([0.;2]);4];let mut lower=[0usize;4];
    for axis in 0..4 {
        let interval=HybridInterval::new(input[axis])?;
        if interval.0[0]<0. || interval.0[1]>1. {return None;}
        let coordinate=interval.op(HybridInterval::point((n-1) as f32)?,2,work,cancelled)?;
        let coordinate=HybridInterval::new([coordinate.0[0].max(0.),coordinate.0[1].min((n-1) as f32)])?;
        let lo=(coordinate.0[0].floor() as usize).min(n-2);let hi=(coordinate.0[1].floor() as usize).min(n-2);
        // Multi-cell boxes require subdivision by the caller, never corner-only
        // extrapolation across a different cell or a different LUT vertex set.
        if lo!=hi {return None;}lower[axis]=lo;
        let fraction=coordinate.op(HybridInterval::point(lo as f32)?,1,work,cancelled)?;
        fractions[axis]=HybridInterval::new([fraction.0[0].max(0.),fraction.0[1].min(1.)])?;
    }
    let one=HybridInterval::point(1.)?;
    let mut result=[[f32::INFINITY,f32::NEG_INFINITY];3];let mut covered=false;
    for order in [[1,2,3],[1,3,2],[2,1,3],[2,3,1],[3,1,2],[3,2,1]] {
        if cancelled() {return None;}*work=work.checked_sub(4)?;
        if fractions[order[0]].0[1]<fractions[order[1]].0[0] || fractions[order[1]].0[1]<fractions[order[2]].0[0] {continue;}
        let weights=[one.op(fractions[order[0]],1,work,cancelled)?,fractions[order[0]].op(fractions[order[1]],1,work,cancelled)?,fractions[order[1]].op(fractions[order[2]],1,work,cancelled)?,fractions[order[2]]];
        let mut sum=[HybridInterval::point(0.)?;3];
        for side in 0..2 {
            let mut point=lower;point[0]+=side;
            let outer=if side==0 {one.op(fractions[0],1,work,cancelled)?} else {fractions[0]};
            for step in 0..4 {
                if cancelled() {return None;}*work=work.checked_sub(1)?;
                let base=(((point[0]*n+point[1])*n+point[2])*n+point[3])*3;
                for channel in 0..3 {
                    let term=outer.op(weights[step],2,work,cancelled)?.op(HybridInterval::point(*clut.get(base+channel)?)?,2,work,cancelled)?;
                    sum[channel]=sum[channel].op(term,0,work,cancelled)?;
                }
                if step<3 {point[order[step]]+=1;}
            }
        }
        covered=true;
        for channel in 0..3 {result[channel][0]=result[channel][0].min(sum[channel].0[0]);result[channel][1]=result[channel][1].max(sum[channel].0[1]);}
    }
    if !covered || cancelled() {return None;}Some(result)
}

#[cfg(all(test,feature="any_to_any"))]
mod hybrid_bound_tests {
    use super::*;
    fn stage()->KatanaLut4x3<f32> {
        let mut stage=KatanaLut4x3::<f32>::default();stage.grid_size=3;
        for c in 0..3 {for m in 0..3 {for y in 0..3 {for k in 0..3 {
            let [c,m,y,k]=[c,m,y,k].map(|v|v as f32/2.);
            stage.clut.extend([c/4.+m/8.+y/16.+k/32.,0.25,(c+m+y+k)/4.]);
        }}}}stage
    }
    #[test]
    fn hybrid_bounds_cover_actual_fetch_and_affine_reference_across_order_changes() {
        let stage=stage();
        for input in [[[0.13,0.14],[0.30,0.31],[0.35,0.36],[0.40,0.41]],[[0.13,0.14],[0.30,0.40],[0.35,0.45],[0.32,0.42]]] {
            let bounds=stage.hybrid_fetch_bounds(input,&mut 10000,&||false).unwrap();
            for sample in 0..1024 {
                let point=std::array::from_fn::<_,4,_>(|axis| {let fraction=((sample*(axis*101+37)+axis*17)%1024) as f32/1023.;input[axis][0]+fraction*(input[axis][1]-input[axis][0])});
                let actual=stage.hybrid_fetch(point[0],point[1],point[2],point[3]).v;
                let ideal=[point[0]/4.+point[1]/8.+point[2]/16.+point[3]/32.,0.25,point.into_iter().sum::<f32>()/4.];
                for channel in 0..3 {assert!(bounds[channel][0]<=actual[channel] && actual[channel]<=bounds[channel][1]);assert!(bounds[channel][0]<=ideal[channel] && ideal[channel]<=bounds[channel][1]);}
            }
        }
    }
    #[test]
    fn hybrid_bounds_refuse_cell_crossings_invalid_short_work_and_cancel() {
        assert_eq!(hybrid_next_up(0.).to_bits(),1);assert_eq!(hybrid_next_up(-0.).to_bits(),1);
        assert_eq!(hybrid_next_down(0.).to_bits(),0x80000001);assert_eq!(hybrid_next_down(-0.).to_bits(),0x80000001);
        assert_eq!(hybrid_next_down(f32::INFINITY),f32::MAX);assert_eq!(hybrid_next_up(f32::NEG_INFINITY),-f32::MAX);
        assert_eq!(hybrid_next_up(f32::MAX),f32::INFINITY);assert_eq!(hybrid_next_down(-f32::MAX),f32::NEG_INFINITY);
        let stage=stage();let input=[[0.13,0.14],[0.30,0.40],[0.35,0.45],[0.32,0.42]];
        let mut work=10000;let polls=std::cell::Cell::new(0);
        stage.hybrid_fetch_bounds(input,&mut work,&||{polls.set(polls.get()+1);false}).unwrap();
        for budget in 0..10000-work {assert!(stage.hybrid_fetch_bounds(input,&mut {budget},&||false).is_none());}
        for stop in 1..=polls.get() {let n=std::cell::Cell::new(0);assert!(stage.hybrid_fetch_bounds(input,&mut 10000,&||{n.set(n.get()+1);n.get()>=stop}).is_none());}
        for bad in [[0.49,0.51],[-0.1,0.1],[0.2,0.1],[f32::NAN,0.3],[0.1,f32::INFINITY]] {let mut v=input;v[0]=bad;assert!(stage.hybrid_fetch_bounds(v,&mut 10000,&||false).is_none());}
        assert!(hybrid_clut_bounds(&stage.clut[..3],3,input,&mut 10000,&||false).is_none());
    }
    #[test]
    fn curve_intervals_cover_knots_nonmonotone_tables_and_shared_diff_rounding() {
        for table in [vec![0.,0.75,0.125,1.],vec![0.,1.],vec![0.25],vec![-1.,0.5,1.5,0.]] {
            for input in [[0.1,0.9],[-0.1,1.1],[0.499999,0.500001],[0.,0.],[1.,1.]] {
                let bounds=hybrid_curve_bounds(&table,input,&mut 10000,&||false).unwrap();
                for index in 0..1024 {let x=input[0]+(input[1]-input[0])*(index as f32/1023.);let actual=lut_interp_linear_float(x,&table);assert!(bounds[0]<=actual && actual<=bounds[1],"input={input:?} bounds={bounds:?} actual={actual}");}
            }
        }
        let table=[0.,0.75,0.125,1.];let mut work=10000;let polls=std::cell::Cell::new(0);
        hybrid_curve_bounds(&table,[0.1,0.9],&mut work,&||{polls.set(polls.get()+1);false}).unwrap();
        for budget in 0..10000-work {assert!(hybrid_curve_bounds(&table,[0.1,0.9],&mut {budget},&||false).is_none());}
        for stop in 1..=polls.get() {let n=std::cell::Cell::new(0);assert!(hybrid_curve_bounds(&table,[0.1,0.9],&mut 10000,&||{n.set(n.get()+1);n.get()>=stop}).is_none());}
        assert!(hybrid_curve_bounds(&[],[0.,1.],&mut 10000,&||false).is_none());
        assert!(hybrid_curve_bounds(&[0.,f32::NAN],[0.,1.],&mut 10000,&||false).is_none());
        assert!(hybrid_curve_bounds(&table,[0.5,0.1],&mut 10000,&||false).is_none());
    }
    #[test]
    fn hybrid_initial_stage_bounds_cover_input_curves_clut_and_output_curves() {
        let mut stage=stage();stage.rrrah_cmyk_hybrid=true;
        stage.linearization=std::array::from_fn(|_|vec![0.,0.5,1.]);
        stage.output=std::array::from_fn(|_|vec![0.,0.25,1.]);
        let input=[[0.13,0.14],[0.30,0.40],[0.35,0.45],[0.32,0.42]];
        let bounds=stage.hybrid_to_pcs_bounds(input,&mut 10000,&||false).unwrap();
        for sample in 0..1024 {
            let point=std::array::from_fn::<_,4,_>(|axis|input[axis][0]+(input[axis][1]-input[axis][0])*(((sample*(axis*101+37))%1024) as f32/1023.));
            let actual=stage.to_pcs(&point).unwrap();
            for channel in 0..3 {assert!(bounds[channel][0]<=actual[channel] && actual[channel]<=bounds[channel][1]);}
        }
        let mut work=10000;stage.hybrid_to_pcs_bounds(input,&mut work,&||false).unwrap();
        for budget in 0..10000-work {assert!(stage.hybrid_to_pcs_bounds(input,&mut {budget},&||false).is_none());}
        assert!(stage.hybrid_to_pcs_bounds(input,&mut 10000,&||true).is_none());
        stage.rrrah_cmyk_hybrid=false;assert!(stage.hybrid_to_pcs_bounds(input,&mut 10000,&||false).is_none());
    }
    #[test]
    fn pcs_matrix_and_lab_branch_bounds_cover_original_stages() {
        use crate::conversions::katana::KatanaIntermediateStage;
        use crate::conversions::katana::KatanaMatrixStage;
        use crate::conversions::katana::KatanaStageLabToXyz;
        let srgb=crate::ColorProfile::new_srgb();
        let unsupported=srgb.create_transform_f32(crate::Layout::Rgb,&srgb,crate::Layout::Rgb,TransformOptions::default()).unwrap();
        assert!(unsupported.bound_float_cmyk_to_rgb([[0.1,0.2];4],&mut 10000,&||false).is_none());
        let matrix=KatanaMatrixStage::new(crate::Matrix3f {v:[[1.,-0.25,0.125],[-0.5,1.,0.25],[0.25,0.,1.]]});
        let lab=KatanaStageLabToXyz::default();
        let empty=KatanaMatrixStage {matrices:Vec::new()};
        assert_eq!(empty.bound_float_pcs([[0.,1.];3],&mut 3,&||false),Some([[0.,1.];3]));
        assert!(empty.bound_float_pcs([[1.,0.];3],&mut 3,&||false).is_none());

        for stage in [&matrix as &dyn KatanaIntermediateStage<f32>,&lab] {
            for input in [[[0.05,0.15],[0.49,0.51],[0.49,0.51]],[[0.5,0.6],[0.6,0.7],[0.6,0.7]]] {
                let bounds=stage.bound_float_pcs(input,&mut 10000,&||false).unwrap();
                for sample in 0..1024 {
                    let mut point:Vec<_>=(0..3).map(|axis|input[axis][0]+(input[axis][1]-input[axis][0])*(((sample*(axis*101+37))%1024) as f32/1023.)).collect();
                    let actual=stage.stage(&mut point).unwrap();
                    for channel in 0..3 {assert!(bounds[channel][0]<=actual[channel] && actual[channel]<=bounds[channel][1]);}
                }
                let mut work=10000;stage.bound_float_pcs(input,&mut work,&||false).unwrap();
                for budget in 0..10000-work {assert!(stage.bound_float_pcs(input,&mut {budget},&||false).is_none());}
                assert!(stage.bound_float_pcs(input,&mut 10000,&||true).is_none());
            }
        }
    }
    #[test]
    #[ignore="Requires RRRAH_CMYK_PROFILE pointing at the renderer's retained ICC"]
    fn retained_cgats_hybrid_clut_box_control() {
        let bytes=std::fs::read(std::env::var("RRRAH_CMYK_PROFILE").unwrap()).unwrap();
        let profile=crate::ColorProfile::new_from_slice(&bytes).unwrap();
        let crate::LutWarehouse::Lut(lut)=profile.lut_a_to_b_perceptual.as_ref().unwrap() else {panic!("Expected retained4D LUT");};
        let lut=make_lut_4x3(lut,TransformOptions::default(),profile.pcs).unwrap();
        let stage=KatanaLut4x3::<f32> {linearization:lut.linearization,clut:lut.clut,grid_size:lut.grid_size,output:lut.output,pcs:lut.pcs,rrrah_cmyk_hybrid:true,..Default::default()};
        let native=[0.18747246773732185,0.658300403967509,0.7777769106931612,0.13612609971991962].map(|v|v as f32);
        let center=std::array::from_fn::<_,4,_>(|i|lut_interp_linear_float(native[i],&stage.linearization[i]));
        // This is a closed CLUT-input box after input curves, not a claim that
        // native source uncertainty has already been propagated through curves.
        let input=center.map(|v|[hybrid_next_down(v-1e-6).max(0.),hybrid_next_up(v+1e-6).min(1.)]);
        let mut work=100000;let bounds=stage.hybrid_fetch_bounds(input,&mut work,&||false).unwrap();
        for sample in 0..4096 {
            let point=std::array::from_fn::<_,4,_>(|axis| {let fraction=((sample*(axis*101+37)+axis*17)%4096) as f32/4095.;input[axis][0]+fraction*(input[axis][1]-input[axis][0])});
            let actual=stage.hybrid_fetch(point[0],point[1],point[2],point[3]).v;
            for channel in 0..3 {assert!(bounds[channel][0]<=actual[channel] && actual[channel]<=bounds[channel][1]);}
        }
        let native_box=native.map(|v|[hybrid_next_down(v-2.3902077e-7).max(0.),hybrid_next_up(v+2.3902077e-7).min(1.)]);
        let mut stage_work=100000;let pcs=stage.hybrid_to_pcs_bounds(native_box,&mut stage_work,&||false).unwrap();
        for sample in 0..4096 {
            let point=std::array::from_fn::<_,4,_>(|axis|native_box[axis][0]+(native_box[axis][1]-native_box[axis][0])*(((sample*(axis*101+37))%4096) as f32/4095.));
            let actual=stage.to_pcs(&point).unwrap();
            for channel in 0..3 {assert!(pcs[channel][0]<=actual[channel] && actual[channel]<=pcs[channel][1]);}
        }
        let executor=profile.create_transform_f32(crate::Layout::Rgba,&crate::ColorProfile::new_srgb(),crate::Layout::Rgb,
            TransformOptions {rrrah_cmyk_hybrid:true,..Default::default()}).unwrap();
        let mut rgb_work=100000;let rgb=executor.bound_float_cmyk_to_rgb(native_box,&mut rgb_work,&||false).unwrap();
        for sample in 0..4096 {
            let point=std::array::from_fn::<_,4,_>(|axis|native_box[axis][0]+(native_box[axis][1]-native_box[axis][0])*(((sample*(axis*101+37))%4096) as f32/4095.));
            let mut actual=[0.;3];executor.transform(&point,&mut actual).unwrap();
            for channel in 0..3 {assert!(rgb[channel][0]<=actual[channel] && actual[channel]<=rgb[channel][1]);}
        }
        let used=100000-rgb_work;
        for budget in 0..used {assert!(executor.bound_float_cmyk_to_rgb(native_box,&mut {budget},&||false).is_none());}
        let polls=std::cell::Cell::new(0);executor.bound_float_cmyk_to_rgb(native_box,&mut 100000,&||{polls.set(polls.get()+1);false}).unwrap();
        for stop in 1..=polls.get() {let n=std::cell::Cell::new(0);assert!(executor.bound_float_cmyk_to_rgb(native_box,&mut 100000,&||{n.set(n.get()+1);n.get()>=stop}).is_none());}
        eprintln!("RETAINED_CGATS_RGB input={native_box:?} rgb={rgb:?} work={used} checked_points=4096 alpha_display_certified=false");
        eprintln!("RETAINED_CGATS_INITIAL_STAGE input={native_box:?} pcs={pcs:?} work={} checked_points=4096 rgb_certified=false",100000-stage_work);
        eprintln!("RETAINED_CGATS_HYBRID_CLUT grid={} input={input:?} bounds={bounds:?} work={} checked_points=4096 end_to_end=false",stage.grid_size,100000-work);
    }

}
