#!/usr/bin/env python3
"""Precommit positive controls before the position-floor RSS outcome is available."""
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
positive=D/'dedup-native-bank-all-positives-prefix-0351-audit-20261011.json';audit=json.loads(positive.read_text());captured=Path(audit['captured_report']);r=json.loads(captured.read_text())
assert all(sha(p)==v for p,v in audit['pins'].items())
chosen=[row for row in r['results'][:351] if row['returncode']==0 and row['evidence']['support'] is True and row['evidence']['model_attempted'] is True][:2]
assert len(chosen)==2
plan=[];pins={str(p):sha(p) for p in [positive,captured,Path(__file__)]}
for row in chosen:
    item=r['plan'][row['index']];paths=[Path(item[n]) for n in ['left','right']];pins.update({str(p):sha(p) for p in paths})
    plan.append(dict(index=row['index'],baseline_index=row['baseline_index'],left=item['left'],right=item['right'],source_tolerance=item['source_tolerance'],historical_model_attempted=True,historical_supported=True))
out=D/'dedup-loftr-position-positive-plan-20261012.json';assert not out.exists()
out.write_text(json.dumps(dict(status='precommitted_two_positive_model_controls',selection='First two completed supported rows requiring model in the independently verified351-prefix fixed order. Selected before position-floor full RSS outcome.',plan=plan,pins=pins,scope='Positive parity controls for baseline/current and isolated candidate full caller; historical model support is not proof the current direct caller supports them. Both executables must be measured and compared. Not all458 qualification.'),indent=2)+'\n');print(plan)
