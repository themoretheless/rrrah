/*
 * // Copyright (c) Radzivon Bartoshyk 6/2025. All rights reserved.
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
use crate::conversions::katana::KatanaIntermediateStage;
use crate::{CmsError, Lab, Xyz};

#[derive(Default)]
pub(crate) struct KatanaStageLabToXyz {}

impl KatanaIntermediateStage<f32> for KatanaStageLabToXyz {
    fn bound_float_pcs(&self,input:[[f32;2];3],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[[f32;2];3]> {
        use crate::conversions::lut4::{HybridInterval as I,hybrid_next_down,hybrid_next_up};
        fn inverted(t:I,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<I> {
            let threshold=24f32/116.;let mut out=[f32::INFINITY,f32::NEG_INFINITY];
            if t.0[0]<=threshold {
                let part=I::new([t.0[0],t.0[1].min(threshold)])?;
                let value=part.op(I::point(16f32/116.)?,1,work,cancelled)?.op(I::point(108f32/841.)?,2,work,cancelled)?.0;
                out=value;
            }
            if t.0[1]>threshold {
                let part=I::new([t.0[0].max(threshold),t.0[1]])?;
                let value=part.op(part,2,work,cancelled)?.op(part,2,work,cancelled)?.0;
                out=[out[0].min(value[0]),out[1].max(value[1])];
            }
            I::new(out)
        }
        if cancelled() {return None;}*work=work.checked_sub(6)?;
        let l=I::new(input[0])?.op(I::point(100.)?,2,work,cancelled)?;
        let a=I::new(input[1])?.op(I::point(255.)?,2,work,cancelled)?.op(I::point(-128.)?,0,work,cancelled)?;
        let b=I::new(input[2])?.op(I::point(255.)?,2,work,cancelled)?.op(I::point(-128.)?,0,work,cancelled)?;
        let y=l.op(I::point(16.)?,0,work,cancelled)?.op(I::point(116.)?,3,work,cancelled)?;
        let x=y.op(a.op(I::point(0.002)?,2,work,cancelled)?,0,work,cancelled)?;
        let z=y.op(b.op(I::point(-0.005)?,2,work,cancelled)?,0,work,cancelled)?;
        const WP:crate::Xyz=crate::Chromaticity::D50.to_xyz();
        let xyz=[inverted(x,work,cancelled)?.op(I::point(WP.x)?,2,work,cancelled)?,inverted(y,work,cancelled)?.op(I::point(WP.y)?,2,work,cancelled)?,inverted(z,work,cancelled)?.op(I::point(WP.z)?,2,work,cancelled)?];
        let mut out=[[0.;2];3];
        for axis in 0..3 {
            if cancelled() {return None;}*work=work.checked_sub(1)?;
            let scale=1f64+32767f64/32768.;
            out[axis]=I::new([hybrid_next_down((f64::from(xyz[axis].0[0])/scale) as f32),hybrid_next_up((f64::from(xyz[axis].0[1])/scale) as f32)])?.0;
        }
        if cancelled() {return None;}Some(out)
    }

    fn stage(&self, input: &mut Vec<f32>) -> Result<Vec<f32>, CmsError> {
        for dst in input.chunks_exact_mut(3) {
            let lab = Lab::new(dst[0], dst[1], dst[2]);
            let xyz = lab.to_pcs_xyz();
            dst[0] = xyz.x;
            dst[1] = xyz.y;
            dst[2] = xyz.z;
        }
        Ok(std::mem::take(input))
    }
}

#[derive(Default)]
pub(crate) struct KatanaStageXyzToLab {}

impl KatanaIntermediateStage<f32> for KatanaStageXyzToLab {
    fn stage(&self, input: &mut Vec<f32>) -> Result<Vec<f32>, CmsError> {
        for dst in input.chunks_exact_mut(3) {
            let xyz = Xyz::new(dst[0], dst[1], dst[2]);
            let lab = Lab::from_pcs_xyz(xyz);
            dst[0] = lab.l;
            dst[1] = lab.a;
            dst[2] = lab.b;
        }
        Ok(std::mem::take(input))
    }
}
