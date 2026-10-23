from pathlib import Path
import subprocess,re,json,hashlib
root=Path(__file__).resolve().parents[3];src=root/".dedup-work/mail092-20261024";python=root/".dedup-work/mail092-runtime-20261024/bin/python";fixture=src/"tests/fixtures/eml-box";files=sorted(fixture.glob("*.eml"));assert len(files)==4
before={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in files};results={}
for jobs in (1,2):
 p=subprocess.run([str(python),"-m","mail_deduplicate","--no-config","--color=never",f"--jobs={jobs}","--hash-only","--hash-body=raw",str(fixture)],cwd=src,text=True,capture_output=True,timeout=30)
 assert p.returncode==0,(p.stdout,p.stderr)
 hashes=re.findall(r"^Hash: ([a-f0-9]+)$",p.stdout,re.M);assert len(hashes)==4 and len(set(hashes))==3;assert hashes[0]==hashes[1];results[str(jobs)]=dict(exit_code=0,hashes=hashes)
assert results["1"]["hashes"]==results["2"]["hashes"]
after={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in files};assert before==after
result=dict(scope="Unmodified package CLI, original four EML fixtures read-only, no-config, raw body hashing, jobs1/2",results=results,fixture_bytes_preserved=True,unique_displayed_header_hashes=3,limits=["Four fixed original messages; not arbitrary corpus or exact-byte equality proof.","Hash-only skips selection/actions and prints header hash_key, not the composite header-plus-body group key."])
(root/"docs/research/dedup-500/mail092-eml-fixture-result-20261024.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
