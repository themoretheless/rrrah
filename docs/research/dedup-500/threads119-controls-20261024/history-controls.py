import pathlib,importlib.util,tempfile,json
base=pathlib.Path(__file__).resolve().parent;spec=importlib.util.spec_from_file_location('dedup_original',base/'02-dedup.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
root=pathlib.Path(tempfile.mkdtemp(prefix='history-',dir=pathlib.Path(__file__).resolve().parent));p=root/'interleaved.json';a=m.DedupChecker(p);b=m.DedupChecker(p);save=a.save
# Deterministic overlap: a has read old history; b completes record before a writes.
def interleave(history):b.record({'affiliate_link':'B'});save(history)
a.save=interleave;a.record({'affiliate_link':'A'})
assert a.load()==[{'affiliate_link':'A'}]
q=root/'serial.json';c=m.DedupChecker(q);c.record({'affiliate_link':'A'});c.record({'affiliate_link':'B'});assert len(c.load())==2
print(json.dumps({'controls_passed':2,'findings':['Deterministically interleaved original record methods lose completed B record when stale A save finishes','Serial original record calls preserve A and B'],'scope':'Original module, owned filesystem, injected scheduling via save wrapper;not threaded/process stress test or posting','root':str(root)},indent=2))
