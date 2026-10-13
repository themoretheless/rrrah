#!/usr/bin/env python3
import hashlib,json,re,subprocess
from collections import Counter
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'dedup-loftr-full-masks-red-20261012.json';gp=D/'dedup-loftr-full-masks-green-20261012.json';red=json.loads(rp.read_text());green=json.loads(gp.read_text());assert red['status']=='reproduced_four_current_source_failures' and green['returncode']==0
 for m in [red,green]:assert all(sha(p)==h for p,h in m['pins'].items())
 mp=D/'dedup-loftr-full-masks-source-20261012.json';m=json.loads(mp.read_text());parent=Path(m['parent_manifest']);b=json.loads(parent.read_text());s=Path(m['snapshot']);base=Path(b['snapshot'])
 assert set(m['deltas_from_parent'])=={'vendor/loftr/src/linear_attention.rs','vendor/loftr/src/linear_attention/tests.rs'}
 for manifest in [m,b]:assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
 for p,h in b['file_hashes'].items():
  if p.startswith(('vendor/loftr/src/','crates/rrrah-dedup/src/')):assert sha(R/p)==h,p
 controls=D/'dedup-loftr-full-mask-controls-20261012.rs';body=D/'dedup-loftr-full-mask-body-20261012.rs';code=(base/'vendor/loftr/src/linear_attention.rs').read_text();start=code.index('        let mut qk = Tensor::einsum("nlhd,nshd->nlsh"');end=code.index('        if self.use_dropout',start);expected='// Modified by rrrah (2026-10-12): independent full-attention masks and finite empty support.\n'+code[:start]+body.read_text()+code[end:]
 fmt=['rustfmt','--edition','2024','--config-path',str(R/'vendor/loftr/rustfmt.toml'),'--emit','stdout'];formatted=subprocess.run(fmt,input=expected,cwd='/tmp',capture_output=True,text=True,check=True).stdout;assert formatted==(s/'vendor/loftr/src/linear_attention.rs').read_text()
 expected=(base/'vendor/loftr/src/linear_attention/tests.rs').read_text()+controls.read_text();assert subprocess.run(fmt,input=expected,cwd='/tmp',capture_output=True,text=True,check=True).stdout==(s/'vendor/loftr/src/linear_attention/tests.rs').read_text()
 text=Path(green['log']).read_text();passed=Counter(re.findall(r'^test (\S+) \.\.\. ok$',text,re.M));assert sum(passed.values())==55 and '55 passed; 0 failed; 0 ignored' in text;assert set(red['failed']).issubset(passed)
 redtext=Path(red['log']).read_text();assert '6 passed; 4 failed; 0 ignored' in redtext
 listed=subprocess.check_output(green['command'][:3]+['--list'],text=True);assert set(passed)==set(re.findall(r'^(.*): test$',listed,re.M));raw=Path(green['command'][2]).read_bytes();assert raw[:4]==b'\xcf\xfa\xed\xfe' and int.from_bytes(raw[4:8],'little')==0x100000c
 out=D/'dedup-loftr-full-masks-verification-20261012.json';assert not out.exists();pins=dict(green['pins']);pins.update({str(p):sha(p) for p in [rp,gp,mp,parent,controls,body,Path(__file__)]});out.write_text(json.dumps({'status':'verified_four_red_then_55_green_native_backend_tests_not_yet_adopted','passed':55,'ignored':0,'red_failures':red['failed'],'tests':sorted(passed),'pins':pins,'scope':'Independent FullAttention masks and unsupported rows reproduced in four failing controls. Expected source plus formatter independently reconstructed; complete native backend suite passes including finite gradients and borrowed-value controls. Unmasked path arithmetic unchanged. No Windows/LinuxTorch/fullformats/full500/totalRSS or corpus qualification.'},indent=2)+'\n');print('Verified four failures before fix,55 passes after fix, zero ignored')
if __name__=='__main__':main()
