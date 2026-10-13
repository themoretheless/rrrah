#!/usr/bin/env python3
"""Precommit visually inspected architecture noncopy controls before runtime."""
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-disc21-architecture-controls-prepared-20261012.json';assert not out.exists()
 parent=D/'dedup-disc21-cohort-prepared-20261011.json';c=json.loads(parent.read_text());assert all(sha(p)==h for p,h in c['pins'].items());images={x['id']:x for x in c['images']}
 pairs=[('R014120','R020965','Different buildings: low unfinished house behind fence versus curved high-rise tower.'),('R020965','R014120','Reverse of the same distinct-building comparison.'),('Q00061','R014120','Grayscale street collage contains high-rise tower R020965; wrong reference is low unfinished house R014120.'),('Q00906','R020965','Dark phone screenshot contains unfinished house R014120; wrong reference is high-rise tower R020965.')]
 plan=[{'query':q,'reference':r,'published_label':'no_reference_match','annotation':note}for q,r,note in pairs];selected=[images[k]for k in sorted({k for q,r,_ in pairs for k in [q,r]})]
 pins={str(parent):sha(parent),str(Path(__file__)):sha(__file__)};pins.update({x['path']:x['sha256']for x in selected})
 out.write_text(json.dumps({'status':'prepared_four_visually_inspected_architecture_noncopy_controls','plan':plan,'images':selected,'pins':pins,'selection':'Four fixed development comparisons chosen by visible architectural content before matching outcomes. Original reference contact sheet and full Q00061/Q00906 images inspected. Reference buildings differ in shape, scale, viewpoint and construction state; full source/decode/provenance retained in parent.','scope':'Manually inspected development noncopy labels, not publisher query-reference negative annotations for reference-reference directions, not independent held-out hard-negative/burst corpus. No image transforms, manual ROIs or threshold changes.'},indent=2)+'\n');print('Prepared four original-JPEG architecture controls')
if __name__=='__main__':main()
