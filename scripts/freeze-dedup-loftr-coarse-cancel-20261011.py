#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-loftr-early-release-snapshot-20261011.json';m=json.loads(parent.read_text());src=Path(m['snapshot']);dst=T/'rrrah-loftr-coarse-cancel-snapshot-20261011';assert not dst.exists() and all(sha(src/f)==h for f,h in m['file_hashes'].items());adoption=json.loads((D/'dedup-loftr-early-release-adoption-20261011.json').read_text());assert all(sha(R/f)==h for f,h in adoption['adopted'].items());shutil.copytree(src,dst)
 f='vendor/loftr/src/loftr_model.rs';q=dst/f;q.write_bytes((R/f).read_bytes());s=q.read_text();needle='let coarse = self.coarse_matching.forward(';assert s.count(needle)==1;s=s.replace(needle,'let coarse = self.coarse_matching.forward_cancellable(');start=s.index('let coarse = self.coarse_matching.forward_cancellable(');end=s.index('\n        )?;',start);block=s[start:end];assert block.endswith('            None,');s=s[:start]+block+'\n            cancel,'+s[end:];q.write_text(s)
 f='vendor/loftr/src/coarse_matching.rs';q=dst/f;s=q.read_text();needle='''        validate_coarse_matching_inputs(feat_c0, feat_c1, data, mask_c0, mask_c1)?;''';replacement='''        self.forward_cancellable(feat_c0, feat_c1, data, mask_c0, mask_c1, &|| false)
    }

    /// Cooperatively cancel between dense tensor operations and softmax blocks.
    /// A running individual libtorch kernel cannot be interrupted.
    #[allow(clippy::too_many_arguments)]
    pub fn forward_cancellable(
        &self,
        feat_c0: &Tensor,
        feat_c1: &Tensor,
        data: &CoarseMatchingData,
        mask_c0: Option<&Tensor>,
        mask_c1: Option<&Tensor>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<CoarseMatchingOutput, LoftrError> {
        crate::error::check_cancel(cancel)?;
'''+needle;assert s.count(needle)==1;s=s.replace(needle,replacement)
 for needle in ['        let mut sim_matrix = match self.config.match_type {','        if let (Some(mask_c0), Some(mask_c1)) = (mask_c0, mask_c1) {','        let threshold_mask =','        let match_indices = matches.nonzero();','        let pred_mask = mconf.ne(0.0);','        Ok(CoarseMatchingOutput {']:
  assert s.count(needle)==1;s=s.replace(needle,'        crate::error::check_cancel(cancel)?;\n'+needle)
 s=s.replace('let conf_matrix = dual_softmax_confidence(&sim_matrix)?;','crate::error::check_cancel(cancel)?;\n        let conf_matrix = dual_softmax_confidence_cancellable(&sim_matrix, cancel)?;')
 needle='''fn dual_softmax_confidence(sim: &Tensor) -> Result<Tensor, LoftrError> {
    let mut column = sim.f_softmax(1, Kind::Float)?;''';replacement='''#[cfg(test)]
fn dual_softmax_confidence(sim: &Tensor) -> Result<Tensor, LoftrError> {
    dual_softmax_confidence_cancellable(sim, &|| false)
}

fn dual_softmax_confidence_cancellable(
    sim: &Tensor,
    cancel: &dyn Fn() -> bool,
) -> Result<Tensor, LoftrError> {
    crate::error::check_cancel(cancel)?;
    let mut column = sim.f_softmax(1, Kind::Float)?;
    crate::error::check_cancel(cancel)?;''';assert s.count(needle)==1;s=s.replace(needle,replacement).replace('''        Ok(column * row)''','''        crate::error::check_cancel(cancel)?;
        Ok(column * row)''').replace('''        while start < rows {
            let count''','''        while start < rows {
            crate::error::check_cancel(cancel)?;
            let count''').replace('''        Ok(column)
    }
}''','''        crate::error::check_cancel(cancel)?;
        Ok(column)
    }
}''');q.write_text(s)
 hashes={f:sha(dst/f) for f in m['file_hashes']};changes=[f for f,h in hashes.items() if h!=m['file_hashes'][f]];assert len(changes)==2;out=D/'dedup-loftr-coarse-cancel-snapshot-20261011.json';assert not out.exists();out.write_text(json.dumps({'snapshot':str(dst),'parent_manifest':str(parent),'parent_sha256':sha(parent),'file_hashes':hashes,'changes':changes,'producer':str(Path(__file__)),'producer_sha256':sha(Path(__file__)),'scope':'Candidate cooperative coarse cancellation, includes qualified model leading-indent cleanup. No individual kernel interruption claim.'},indent=2)+'\n')
if __name__=='__main__':main()
