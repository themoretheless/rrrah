#!/usr/bin/env python3
import urllib.request,hashlib,json,os,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');D=R/'docs/research';S=T/'rrrah-sscd-source-20261013';F=T/'rrrah-sscd-models-20261013';F.mkdir(exist_ok=True)
url='https://dl.fbaipublicfiles.com/sscd-copy-detection/sscd_disc_mixup.torchscript.pt';out=F/'sscd_disc_mixup.torchscript.pt';report=D/'dedup-sscd-acquisition-20261013.json';assert not out.exists() and not report.exists();tmp=out.with_suffix('.download');assert not tmp.exists();maximum=150*1024*1024
with urllib.request.urlopen(url,timeout=60) as response,tmp.open('xb') as f:
 headers=dict(response.headers);n=0
 while True:
  block=response.read(1024*1024)
  if not block:break
  n+=len(block)
  if n>maximum:raise RuntimeError('model byte limit')
  f.write(block)
 assert n==int(headers['Content-Length'])
tmp.rename(out)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();names=subprocess.check_output(['git','ls-files'],cwd=S,text=True).splitlines();pins={n:sha(S/n) for n in names};commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=S,text=True).strip();assert commit=='95902662f2217a5f4aa45f2a3fc70a01dfd3b66a'
report.write_text(json.dumps(dict(status='acquired_for_diagnostic_not_integrated',repository='https://github.com/facebookresearch/sscd-copy-detection',commit=commit,source=str(S),source_sha256=pins,source_files=len(pins),url=url,weights=str(out),weights_sha256=sha(out),weights_bytes=n,headers=headers,license_note='Repository MIT license read; README publishes these weights for reproduction. No separate model-license file present in tree; no license transfer to images. No code or weights adopted into production.',scope='Acquisition and full-tree hash inventory only, not full source review or runtime qualification.'),indent=2)+'\n');print('acquired',n,sha(out))
