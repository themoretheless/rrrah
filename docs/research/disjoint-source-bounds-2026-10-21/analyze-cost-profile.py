#!/usr/bin/env python3
"""Summarize logical work counters; reported category sums are lower bounds."""
from pathlib import Path
import json
import re
import sys

sections=[]
for line in Path(sys.argv[1]).read_text().splitlines():
    if line.startswith('SAMPLE_COST_BEGIN'):
        if sections:
            assert sections[-1]['end'] is not None
        values={k:int(v) for k,v in re.findall(r'\b(patches|blocks)=(\d+)',line)}
        sections.append({**values,'begin':line,'end':None,'tiles':[],'cells':[]})
    elif sections:
        section=sections[-1]
        if line.startswith('TILE_COST'):
            section['tiles'].append(line)
        elif line.startswith('CELL_COST_'):
            section['cells'].append(line)
        elif line.startswith('SAMPLE_COST_END'):
            section['end']=line
            section['complete']='complete=true' in line
            for key in ['spent','remaining']:
                section[key]=int(re.search(r'\b'+key+r'=(\d+)',line)[1])
def value(line,key):
    return int(re.search(r'\b'+key+r'=(\d+)',line)[1])
for section in sections:
    assert section['end'] is not None
    section['reported_tile_work']=sum(value(l,'work') for l in section['tiles'])
    section['reported_cell_paint']=sum(value(l,'paint') for l in section['cells'])
    section['reported_cell_total']=sum(value(l,'total') for l in section['cells'])
    section['unreported_work']=section['spent']-section['reported_tile_work']-section['reported_cell_total']
    assert section['unreported_work']>=0
    section['largest_paint_cells']=sorted(section['cells'],key=lambda l:value(l,'paint'),reverse=True)[:5]
Path(sys.argv[2]).write_text(json.dumps(sections,indent=2)+'\n')
for s in sections:
    print({k:s[k] for k in ['patches','complete','spent','remaining','reported_tile_work','reported_cell_paint','unreported_work']})
