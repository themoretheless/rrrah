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
use crate::{CmsError, TransformExecutor};
use std::marker::PhantomData;

/// W storage working data type
/// I input/output data type
pub(crate) trait KatanaInitialStage<W, I> {
    fn bound_float_cmyk(&self,_input:[[f32;2];4],_work:&mut usize,_cancelled:&dyn Fn()->bool)->Option<[[f32;2];3]> {None}
    fn to_pcs(&self, input: &[I]) -> Result<Vec<W>, CmsError>;
}

/// W storage working data type
/// I input/output data type
pub(crate) trait KatanaFinalStage<W, I> {
    fn bound_float_rgb(&self,_input:[[f32;2];3],_work:&mut usize,_cancelled:&dyn Fn()->bool)->Option<[[f32;2];3]> {None}
    fn to_output(&self, src: &mut [W], dst: &mut [I]) -> Result<(), CmsError>;
}

/// W storage working data type
pub(crate) trait KatanaIntermediateStage<W> {
    fn bound_float_pcs(&self,_input:[[f32;2];3],_work:&mut usize,_cancelled:&dyn Fn()->bool)->Option<[[f32;2];3]> {None}
    fn stage(&self, input: &mut Vec<W>) -> Result<Vec<W>, CmsError>;
}

pub(crate) struct BlackholeIntermediateStage<W> {
    pub(crate) _phantom: PhantomData<W>,
}

impl<W> KatanaIntermediateStage<W> for BlackholeIntermediateStage<W> {
    fn bound_float_pcs(&self,input:[[f32;2];3],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[[f32;2];3]> {
        if cancelled() {return None;}*work=work.checked_sub(4)?;
        for v in input {crate::conversions::lut4::HybridInterval::new(v)?;}
        if cancelled() {return None;}Some(input)
    }
    fn stage(&self, input: &mut Vec<W>) -> Result<Vec<W>, CmsError> {
        Ok(std::mem::take(input))
    }
}

/// I input/output data type
pub(crate) trait KatanaPostFinalizationStage<I> {
    fn preserves_float_rgb(&self,_work:&mut usize,_cancelled:&dyn Fn()->bool)->bool {false}
    fn finalize(&self, src: &[I], dst: &mut [I]) -> Result<(), CmsError>;
}

/// W storage working data type
/// I input/output data type
pub(crate) struct Katana<W, I> {
    pub(crate) initial_stage: Box<dyn KatanaInitialStage<W, I> + Send + Sync>,
    pub(crate) final_stage: Box<dyn KatanaFinalStage<W, I> + Sync + Send>,
    pub(crate) stages: Vec<Box<dyn KatanaIntermediateStage<W> + Send + Sync>>,
    pub(crate) post_finalization: Vec<Box<dyn KatanaPostFinalizationStage<I> + Send + Sync>>,
}

impl<W, I: Copy + Default> TransformExecutor<I> for Katana<W, I> {
    fn bound_float_cmyk_to_rgb(&self,input:[[f32;2];4],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[[f32;2];3]> {
        if cancelled() {return None;}
        let mut values=self.initial_stage.bound_float_cmyk(input,work,cancelled)?;
        for stage in &self.stages {
            if cancelled() {return None;}*work=work.checked_sub(1)?;
            values=stage.bound_float_pcs(values,work,cancelled)?;
        }
        let output=self.final_stage.bound_float_rgb(values,work,cancelled)?;
        for stage in &self.post_finalization {if !stage.preserves_float_rgb(work,cancelled) {return None;}}
        if cancelled() {return None;}Some(output)
    }
    fn transform(&self, src: &[I], dst: &mut [I]) -> Result<(), CmsError> {
        let mut working_vec = self.initial_stage.to_pcs(src)?;
        for stage in self.stages.iter() {
            working_vec = stage.stage(&mut working_vec)?;
        }
        self.final_stage.to_output(&mut working_vec, dst)?;
        for finalization in self.post_finalization.iter() {
            finalization.finalize(src, dst)?;
        }
        Ok(())
    }
}
