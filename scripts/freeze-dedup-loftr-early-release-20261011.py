#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-loftr-row-softmax-snapshot-20261011.json';m=json.loads(p.read_text());src=Path(m['snapshot']);dst=T/'rrrah-loftr-early-release-snapshot-20261011';assert not dst.exists();assert all(sha(src/f)==h for f,h in m['file_hashes'].items());shutil.copytree(src,dst)
 # Include the adopted and already qualified boundary test in the child.
 test='vendor/loftr/src/coarse_matching/tests.rs';q=dst/test;assert (R/test).read_text().startswith(q.read_text());q.write_bytes((R/test).read_bytes())
 f='vendor/loftr/src/loftr_model.rs';q=dst/f;s=q.read_text();start=s.index('            debug: if collect_debug {');end=s.index('\n        })',start);debug=s[start:end];assert debug.endswith('            },');expression=debug.removeprefix('            debug: ').removesuffix(',')
 s=s[:start]+'            debug,'+s[end:]
 marker='''        crate::error::check_cancel(cancel)?;
        let (mut feat_f0_unfold, mut feat_f1_unfold) = self.fine_preprocess.forward('''
 insertion='''        // Debug statistics own only scalars; do not retain dense confidence
        // through fine matching when the coarse decisions are already fixed.
        let debug = '''+expression.strip()+''';
        drop(coarse.conf_matrix);

'''
 assert s.count(marker)==1;s=s.replace(marker,insertion+marker)
 marker='''        crate::error::check_cancel(cancel)?;
        if feat_f0_unfold.size()[0] != 0 {'''
 insertion='''        let hw0_f = coarse_features.hw0_f;
        drop(backbone_features);
        drop(coarse_features);
''';assert s.count(marker)==1;s=s.replace(marker,insertion+marker);needle='''                hw0_f: coarse_features.hw0_f,
                mkpts0_c:''';assert s.count(needle)==1;s=s.replace(needle,'''                hw0_f,
                mkpts0_c:''');q.write_text(s)
 hashes={f:sha(dst/f) for f in m['file_hashes']};changes=[f for f,h in hashes.items() if h!=m['file_hashes'][f]];assert sorted(changes)==sorted([f,test]);out=D/'dedup-loftr-early-release-snapshot-20261011.json';assert not out.exists();out.write_text(json.dumps({'snapshot':str(dst),'parent_manifest':str(p),'parent_sha256':sha(p),'file_hashes':hashes,'changes':changes,'producer':str(Path(__file__)),'producer_sha256':sha(Path(__file__)),'scope':'Candidate releases dense coarse confidence before fine preprocessing, backbone/coarse features before fine transformer. Debug computed before releases. Adopted boundary test included.'},indent=2)+'\n')
if __name__=='__main__':main()
