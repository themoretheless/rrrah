#!/usr/bin/env python3
"""Verify every historical triage record and archived evidence, not algorithm coverage."""
import hashlib,json
from collections import Counter
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 out=D/'source-coverage-audit-20261011.json';assert not out.exists()
 inventory=D/'repositories.json';data=json.loads(inventory.read_text());assert data['count']==500 and len(data['repositories'])==500
 names={x['name'] for x in data['repositories']};assert len(names)==500
 reviews=sorted((D/'reviews').glob('*.json'));assert len(reviews)==500
 pins={str(inventory):sha(inventory),str(Path(__file__)):sha(Path(__file__))};rows=[];errors=[];seen=set();indices=set()
 for p in reviews:
  d=json.loads(p.read_text());repo=d['repository'];assert repo in names and repo not in seen;seen.add(repo);indices.add(d['index']);pins[str(p)]=sha(p)
  directory=D/'evidence'/repo.replace('/','--');files=d.get('available_source_evidence',[])+d.get('available_readme_license_evidence',[])
  for item in files:
   q=directory/item['local'];assert q.parent==directory
   if not q.is_file() or sha(q)!=item['sha256']:errors.append({'repository':repo,'path':str(q),'reason':'missing_or_digest_mismatch'})
   else:pins[str(q)]=sha(q)
  source=d.get('available_source_evidence',[])
  rows.append({'index':d['index'],'repository':repo,'commit':d.get('commit'),'decision':d['decision'],'review_scope':d['review_scope'],'archived_source_files':len(source),'truncated_source_files':sum(bool(x.get('truncated')) for x in source),'upstream_tests_run_in_historical_record':d['upstream_tests_run'],'tree_truncated':d.get('tree_truncated')})
 assert seen==names and indices==set(range(1,501))
 out.write_text(json.dumps({'status':'verified_500_historical_triage_evidence' if not errors else 'failed_evidence_integrity','repositories':500,'decisions':dict(Counter(x['decision'] for x in rows)),'repositories_with_archived_source':sum(x['archived_source_files']>0 for x in rows),'repositories_with_truncated_archived_source':sum(x['truncated_source_files']>0 for x in rows),'archived_source_files':sum(x['archived_source_files'] for x in rows),'errors':errors,'rows':rows,'pins':pins,'scope':'Checks every historical500 record and its selected archived source/readme/license hashes. Does not assert all source files were read, algorithms qualified, upstream suites executed, or library all-case completion. Later deep/full-source followups remain separate evidence.'},indent=2)+'\n');print('500 records; source repositories',sum(x['archived_source_files']>0 for x in rows),'integrity errors',len(errors));assert not errors
if __name__=='__main__':main()
