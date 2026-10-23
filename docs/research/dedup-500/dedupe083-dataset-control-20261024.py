"""Full-row structural/data-label audit of all four original pinned CSVs."""
import collections,csv,hashlib,io,itertools,json,pathlib
root=pathlib.Path(__file__).resolve().parents[3]
w=root/'.dedup-work/dedupe083-20261024'
tree=json.loads((root/'docs/research/dedup-500/evidence/dedupeio--dedupe/tree.json').read_text())['tree']
results=[];normalized={}
for p in sorted((w/'benchmarks/benchmarks/datasets').glob('*.csv')):
 b=p.read_bytes();relative=str(p.relative_to(w));t=next(x for x in tree if x['path']==relative)
 assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==t['sha']
 reader=csv.DictReader(io.StringIO(b.decode('utf8')),skipinitialspace=True)
 assert reader.fieldnames in [['name','address','city','cuisine','unique_id'],['name','address','city','cuisine']]
 rows=[]
 for row in reader:
  assert set(row)==set(reader.fieldnames) and all(v is not None for v in row.values())
  row={k:v.strip().strip('"').strip("'").lower() for k,v in row.items()}
  assert row['name'] and ('unique_id' not in row or row['unique_id']);rows.append(row)
 groups=collections.Counter(r['unique_id'] for r in rows if 'unique_id' in r)
 normalized[p.name]=rows
 results.append({'path':relative,'sha256':hashlib.sha256(b).hexdigest(),'rows':len(rows),'unique_labels':len(groups),'label_group_sizes':dict(sorted(collections.Counter(groups.values()).items())),'labelled_pairs':sum(n*(n-1)//2 for n in groups.values()),'blank_cells':dict((k,sum(not r[k] for r in rows)) for k in reader.fieldnames),'duplicate_full_rows':len(rows)-len({tuple(r.items()) for r in rows}),'method':'Every row parsed and structurally checked; all label groups enumerated, not manual reading of every cell.'})
a=normalized['restaurant-1.csv'];b=normalized['restaurant-2.csv'];ca=collections.Counter(r['unique_id'] for r in a);cb=collections.Counter(r['unique_id'] for r in b)
cross=sum(ca[k]*cb[k] for k in ca.keys()&cb.keys())
assert cross==112
train=normalized['restaurant-nophone-training.csv'];counts=collections.Counter(r['unique_id'] for r in train)
assert sum(v*(v-1)//2 for v in counts.values())==112
print(json.dumps({'datasets':results,'cross_dataset_labelled_pairs':cross,'training_labelled_pairs':112,'limits':['Structured whole-data inspection, not manual full-cell review or independent correctness of human labels.','Does not establish legal dataset provenance or general accuracy.','No whole repository completion asserted.']},indent=2))
