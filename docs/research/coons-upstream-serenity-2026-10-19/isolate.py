#!/usr/bin/env python3
"""Reproduce same-length shading isolation from pinned external parents."""
import json,hashlib,argparse
from pathlib import Path
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('corpus',type=Path);a=parser.parse_args()
p=Path(__file__).resolve().parent
parents=json.loads((p/'sources.json').read_text());derived=json.loads((p/'isolated-sources.json').read_text())
for source in parents['sources']:
 data=(a.corpus/source['name']).read_bytes()
 assert len(data)==source['bytes'] and hashlib.sha256(data).hexdigest()==source['sha256']
 for keep in [1,2]:
  omitted=b'/Sh2 sh' if keep==1 else b'/Sh1 sh'
  assert data.count(omitted)==1
  out=data.replace(omitted,b' '*len(omitted))
  name=source['name'].replace('.pdf',f'-sh{keep}.pdf')
  expected=next(x for x in derived['sources'] if x['name']==name)
  assert len(out)==expected['bytes'] and hashlib.sha256(out).hexdigest()==expected['sha256']
  (a.corpus/name).write_bytes(out)
