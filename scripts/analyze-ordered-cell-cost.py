#!/usr/bin/env python3
"""Summarize opt-in local cell charges; parent retries exclude child work."""
import argparse
from collections import Counter
import json
from pathlib import Path
import re

CELL = re.compile(r'ORDERED_CELL_COST origin=\(([^,]+), ([^)]+)\) side=(\S+) depth=(\d+) limit=(\d+) candidates=(\d+) processed=(\d+) fragments=(\d+) status=(\w+) gather=(\d+) sort=(\d+) paint=(\d+) total=(\d+) left=(\d+)')
STAGES = re.compile(r'ORDERED_STAGE_COST patches=(\d+) scans=(\d+) initial_paint=(\d+) initial_visit=(\d+) dense=(\d+) total=(\d+) left=(\d+)')
OUTCOME = re.compile(r'ORDERED_SAMPLE_OUTCOME patches=(\d+) complete=(true|false) work_spent=(\d+) work_left=(\d+)')


def summarize(rows, outcome=None):
    totals = {k: sum(r[k] for r in rows) for k in ['gather', 'sort', 'paint', 'total']}
    by_status = Counter()
    by_depth = Counter()
    phases_by_status = {}
    for r in rows:
        by_status[r['status']] += r['total']
        by_depth[r['depth']] += r['total']
        phases = phases_by_status.setdefault(r['status'], Counter())
        phases.update({k: r[k] for k in ['gather', 'sort', 'paint']})
    result = dict(logged_cells=len(rows), charges=totals, charges_by_status=dict(by_status),
                  charges_by_depth=dict(by_depth), phase_charges_by_status={k: dict(v) for k, v in phases_by_status.items()}, largest_cells=sorted(rows, key=lambda r: -r['total'])[:20])
    if outcome:
        result['outcome'] = outcome
        result['unattributed_work'] = outcome['work_spent'] - totals['total']
        if result['unattributed_work'] < 0:
            raise ValueError('cell charges exceed continuing page allowance')
    return result


def analyze(text):
    groups = []
    rows = []
    stages = None
    for line in text.splitlines():
        if line.startswith('ORDERED_CELL_COST '):
            m = CELL.fullmatch(line)
            if not m:
                raise ValueError('malformed cost event')
            row = dict(origin=[float(m[1]), float(m[2])], side=float(m[3]), depth=int(m[4]),
                       limit=int(m[5]), candidates=int(m[6]), processed=int(m[7]),
                       fragments=int(m[8]), status=m[9], gather=int(m[10]), sort=int(m[11]),
                       paint=int(m[12]), total=int(m[13]), left=int(m[14]))
            if row['total'] != row['gather'] + row['sort'] + row['paint'] or row['processed'] > row['candidates']:
                raise ValueError('inconsistent cell accounting')
            rows.append(row)
        elif line.startswith('ORDERED_STAGE_COST '):
            m = STAGES.fullmatch(line)
            if not m or stages is not None:
                raise ValueError('malformed or repeated stage totals')
            stages = dict(zip(['patches','scans','initial_paint','initial_visit','dense','total','left'], map(int,m.groups())))
            if sum(stages[k] for k in ['scans','initial_paint','initial_visit','dense']) != stages['total']:
                raise ValueError('inconsistent stage accounting')
        elif line.startswith('ORDERED_SAMPLE_OUTCOME '):
            m = OUTCOME.fullmatch(line)
            if not m:
                raise ValueError('malformed page outcome')
            groups.append(summarize(rows, dict(patches=int(m[1]), complete=m[2] == 'true',
                                              work_spent=int(m[3]), work_left=int(m[4]))))
            if stages is not None:
                outcome = groups[-1]['outcome']
                if stages['patches'] != outcome['patches'] or stages['left'] != outcome['work_left'] or stages['total'] > outcome['work_spent']:
                    raise ValueError('stage totals disagree with outcome')
                groups[-1]['stage_charges'] = stages
                groups[-1]['stage_unattributed_work'] = outcome['work_spent'] - stages['total']
                stages = None
            rows = []
    if stages is not None:
        raise ValueError('stage totals without a terminal outcome')
    if rows:
        groups.append(summarize(rows))
    if not groups:
        raise ValueError('no cost events/outcomes')
    return dict(scope='Charged local cell work, not elapsed time. Threshold-filtered events omit visits, early gathering/sorting failures and other page work.', attempts=groups)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('log', type=Path)
    args = parser.parse_args()
    print(json.dumps(analyze(args.log.read_text()), indent=2))
