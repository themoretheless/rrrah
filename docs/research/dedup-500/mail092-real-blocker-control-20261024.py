from pathlib import Path
from types import SimpleNamespace
import filecmp,os,sys,json,tempfile
root=Path(__file__).resolve().parents[3];sys.path.insert(0,str(root/".dedup-work/mail092-20261024"))
from mail_deduplicate.action import hardlink_blocker
from mail_deduplicate.mail_box import EML
owned=Path(tempfile.mkdtemp(prefix="mail092-real-blocker-",dir=root/".dedup-work"));box=EML(owned/"box");key_a=box.add(b"aaaa");key_b=box.add(b"aaaa");a=Path(box._path)/key_a;b=Path(box._path)/key_b
mail=SimpleNamespace(box=box,path=str(a));target=SimpleNamespace(box=box,path=str(b));dedup=SimpleNamespace(conf=dict(hardlink_differing=False));filecmp.clear_cache()
assert hardlink_blocker(dedup,mail,target) is None
before=b.stat();b.write_bytes(b"bbbb");os.utime(b,ns=(before.st_atime_ns,before.st_mtime_ns));assert a.read_bytes()!=b.read_bytes()
stale=hardlink_blocker(dedup,mail,target);assert stale is None
filecmp.clear_cache();fresh=hardlink_blocker(dedup,mail,target);assert fresh is not None and "differs byte for byte" in fresh
box.close();result=dict(scope="Normally imported unmodified original hardlink_blocker, real EML box and physical owned files; mail records expose actual box/path; no AST adapters",stale_cached_comparison_allows_different_bytes=True,cleared_cache_rejects=True,owned_directory=str(owned),limits=["No hardlink action or replacement invoked.","Control reproduces comparator gate, not full CLI content replacement."])
(root/"docs/research/dedup-500/mail092-real-blocker-result-20261024.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
