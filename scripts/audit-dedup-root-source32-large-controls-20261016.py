"""Independently verify current exact-module results on the competitor's concrete false duplicates."""
from pathlib import Path
import hashlib,json,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-root-source32-large-controls-20261016.json';d=json.loads(P.read_text());assert d['status']=='verified_current_exact_rejects_source32_large_false_duplicates';assert h(R/'scripts/qualify-dedup-root-source32-large-controls-20261016.py')==d['producer_sha256'];W=R/'.dedup-work/root-source32-large-v2-20261016';assert h(W/'exact.rs')==h(R/d['source'])==d['source_sha256'];assert h(R/d['binary'])==d['binary_sha256'] and h(R/d['harness'])==d['harness_sha256'] and h(R/d['build_log'])==d['build_log_sha256'];assert len(d['rows'])==3
for name,digest in d['dependencies'].items():assert h(name)==digest
expected={'2m':2*1024*1024,'4g':2**32,'4g64k':2**32+65536};seen=set()
for row in d['rows']:
 assert row['case'] not in seen and expected[row['case']]==row['size'];seen.add(row['case']);assert row['different_offset']==row['size']//2+123 and row['both_orders_rejected'];log=R/row['log'];assert h(log)==row['log_sha256'];m=re.fullmatch(r'SIZE=(\d+) OFFSET=(\d+) BOTH_ORDERS_REJECTED=true CHECKPOINTS=(\d+)\n',log.read_text());assert m and int(m[1])==row['size'] and int(m[2])==row['different_offset'] and int(m[3])>0
 for name,byte in zip(row['inputs'],[b'X',b'Y']):
  path=R/name;assert path.stat().st_size==row['size']
  with path.open('rb') as f:f.seek(row['different_offset']);assert f.read(1)==byte
out=D/'dedup-root-source32-large-controls-audit-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified6_current_public_exact_comparisons_reject_competitor_false_duplicates','report':str(P.relative_to(R)),'report_sha256':h(P),'auditor_sha256':h(__file__),'source_sha256':d['source_sha256'],'scope':d['scope'],'qualification':'3exactdistinctsizes/bothorders, no fullCargo/decode/currentcollection/platform/RSS/performance claim'},indent=2)+'\n');print('Verified3sizes/bothorders/currentexactsource/rawoutputs/dependencies andindependentdifferingbytes')
