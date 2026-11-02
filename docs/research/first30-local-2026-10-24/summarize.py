import re,json,statistics,sys
from pathlib import Path
for name in sys.argv[1:]:
 groups={}
 for line in Path(name).read_text().splitlines():
  if 'FOREGROUND_PAIR ' not in line and 'TELEMETRY_PAIR ' not in line:continue
  row=dict(re.findall(r'(\w+)=([\w.]+)',line));kind='foreground' if 'producers'in row else 'telemetry';key=(kind,row.get('producers','1'),row['control']);groups.setdefault(key,{}).setdefault(row['pair'],{})[row['mode']]=row
 out={}
 for key,pairs in groups.items():
  fields=[k for k,v in next(iter(pairs.values()))['0'].items() if k.endswith('_ns') or k=='native_peak'];stats={}
  for field in fields:
   ratios=[int(rows['1'][field])/int(rows['0'][field])for rows in pairs.values() if int(rows['0'][field])>0];stats[field]={'median_ratio':statistics.median(ratios) if ratios else None,'wins':sum(x<1 for x in ratios),'pairs':len(ratios),'baseline_median':statistics.median(int(rows['0'][field])for rows in pairs.values()),'candidate_median':statistics.median(int(rows['1'][field])for rows in pairs.values()),'ratios':ratios}
  out['/'.join(key)]=stats
 Path(name+'.json').write_text(json.dumps(out,indent=2)+'\n');print(name)
 for key,stats in out.items():print(key,{field:(round(v['median_ratio'],3) if v['median_ratio'] is not None else None,v['wins'])for field,v in stats.items()})
