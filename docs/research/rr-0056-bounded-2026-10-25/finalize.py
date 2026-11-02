import collections,hashlib,json,re,statistics
from pathlib import Path
root=Path(__file__).resolve().parents[3];out=Path(__file__).resolve().parent;rel=out.relative_to(root).as_posix()
bench=json.loads((out/'benchmarks.json').read_text())
assert len(bench)==3 and all(b['exit_code']==0 and not b['timed_out'] and b['summaries']==[['ok','1','0','0']]for b in bench),bench
checks=json.loads((out/'checks.json').read_text());assert all(c['exit_code']==0 and not c['timed_out']for c in checks)
summary={}
for log in ['pairs-1.log','pairs-2.log']:
 rows=[]
 for line in (out/log).read_text().splitlines():
  if line.startswith('SWAP_TTL_PAIR '):
   row=dict(v.split('=')for v in line.split()[1:]);rows.append({k:(v=='true'if k=='control'else int(v))for k,v in row.items()})
 summary[log]={}
 for consumers in [1,2,4]:
  summary[log][str(consumers)]={}
  for control in [True,False]:
   selected=[r for r in rows if r['consumers']==consumers and r['control']==control]
   assert len(selected)==20
   pairs={i:{v['mode']:v for v in selected if v['pair']==i}for i in range(10)}
   summary[log][str(consumers)][str(control)]={k:dict(median_ratio=statistics.median(p[1][k]/p[0][k]for p in pairs.values()),wins=sum(p[1][k]<p[0][k]for p in pairs.values()))for k in ['wait_p50_ns','wait_p95_ns','hold_p95_ns','cleanup_p95_ns']}
scale=[]
for line in (out/'scaling.log').read_text().splitlines():
 if line.startswith('TTL_SCALING '):
  row=dict(v.split('=')for v in line.split()[1:]);scale.append({k:(v=='true'if k=='candidate'else int(v))for k,v in row.items()})
scaling={}
for count in [129,1025]:
 pairs={i:{v['candidate']:v for v in scale if v['count']==count and v['pair']==i}for i in range(3)}
 scaling[str(count)]=dict(median_cleanup_ratio=statistics.median(p[True]['cleanup_ns']/p[False]['cleanup_ns']for p in pairs.values()),pairs=3)
paths=['crates/rrrah-cache/src/swap.rs','crates/rrrah-memory/src/policy.rs']
qualification=dict(date='2026-10-25',scope='RR-0056 background TTL worker only; fixed-stack values, existing O(N) key plan retained',checks=checks,benchmarks=bench,paired_results=summary,scaling=scaling,source_sha256={p:hashlib.sha256((root/p).read_bytes()).hexdigest()for p in paths},limits=['Local macOS/APFS real files; no hardware qualification','Public prune and lazy get expiry retain previous mutex/quota ordering','Fixed 64-payload array adds bounded stack storage, not a new O(N) heap collection','Existing O(N) expiry key Vec and allocator bookkeeping are not newly brought into MemoryBudget','Mutex microbenchmark synchronizes contenders at detach; not end-to-end viewer throughput','pairs-1 preceded final public-prune compatibility restoration; its detach code is identical, pairs-2 and scaling use final source'])
(out/'qualification.json').write_text(json.dumps(qualification,ensure_ascii=False,indent=2)+'\n')
p=root/'docs/improvements-1000.json';d=json.loads(p.read_text());t=d['tasks'][55];x=t['execution']
x.update(status='verified',updated_at='2026-10-25',result='Фоновый TTL-worker извлекает expired payloads пакетами по 64 в фиксированный массив на стеке и удаляет файлы после освобождения index mutex. Убран дополнительный O(N) payload Vec; прежний O(N) список ключей сохранён. Проверены отмена между пакетами, повторная очистка, свежие замены/pins, внешние handles и возврат quota. Парные замеры 1/2/4 читателей и real-file batches 129/1025 сохранены.',remaining='Проверена локальная CPU/APFS TTL-maintenance область. Публичный prune, lazy get expiry и spill eviction сохраняют прежний mutex/quota порядок; общего ускорения viewer не заявляется. Существующий O(N) key plan не стал учтённым MemoryBudget и остаётся отдельной границей.',report=rel+'/REPORT.md')
x['evidence_files']+= [rel+'/'+name for name in ['memory.log','cache-final.log','checks.json','pairs-1.log','pairs-2.log','scaling.log','benchmarks.json','qualification.json']]
x['evidence_files']=list(dict.fromkeys(x['evidence_files']))
t['public_api']='WeightedLru получает additive expired_keys_for_prune(cutoff) и remove_if_expired_at(key, cutoff). План допускает отмену/перекрытие; удаление перепроверяет срок и protected status. Существующие методы, ImageSwapCache public prune и порядок допуска дисковой квоты сохранены; оптимизация включена только в background TTL-worker.'
for b in d.get('execution_batches',[]):
 if t['id']in b['ids']:
  by_id={t['id']:t for t in d['tasks']};b['summary']=dict(collections.Counter(by_id[i]['execution']['status']for i in b['ids']))
  b['note']='Проверка RR-0031–RR-0060 завершена: 4 внедрённых verified, 26 конкретных гипотез rejected. Rejected не означает выполненную оптимизацию. RR-0056 завершена отдельным bounded-release проходом; VoiceOver и другое оборудование исключены по указанию пользователя.'
p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(dict(pairs=summary,scaling=scaling),indent=2))
