import importlib.util, pathlib, tempfile, hashlib, json, contextlib, io, sys
source=pathlib.Path(__file__).with_name('dedupe.py')
spec=importlib.util.spec_from_file_location('original_dedupe',source)
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
root=pathlib.Path(tempfile.mkdtemp(prefix='owned-',dir=pathlib.Path(__file__).resolve().parent))
p=root/'sample';p.mkdir()
a=p/'long_keeper.mp4';b=p/'b.mp4'
payload=b'A'*65536+b'B'*1000000+b'Z'*65536
a.write_bytes(payload); changed=bytearray(payload);changed[500000]=67;b.write_bytes(changed)
assert hashlib.sha256(a.read_bytes()).digest()!=hashlib.sha256(b.read_bytes()).digest()
assert m.file_fingerprint(a)==m.file_fingerprint(b)
assert len(next(iter(m.scan_performer(p).values())))==2
out=io.StringIO();sys.argv=['dedupe.py','--output-dir',str(root)]
with contextlib.redirect_stdout(out): assert m.main()==0
assert a.exists() and b.exists() and '1 duplicates found' in out.getvalue()
c=p/'identical.webm';c.write_bytes(payload)
assert m.file_fingerprint(a)==m.file_fingerprint(c)
small=p/'tiny.mp4';small.write_bytes(b'x'*99);assert m.file_fingerprint(small) is None
result={'repository':'KevinStreetCoder/harvestr','original_source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'runtime':sys.version,'owned_root':str(root),'controls':[{'name':'different middle bytes produce same candidate fingerprint','passed':True},{'name':'original dry-run reports false duplicate without full-byte confirmation','passed':True},{'name':'identical bytes same fingerprint positive','passed':True},{'name':'below-minimum-size ignored','passed':True}],'dry_run_stdout':out.getvalue(),'scope':'Original Python module, real owned files; dry-run only; no upstream tests or downloader runtime'}
print(json.dumps(result,indent=2))
