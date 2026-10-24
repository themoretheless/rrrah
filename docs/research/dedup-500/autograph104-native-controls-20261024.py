import tempfile, pathlib, json, common, cleanup
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-owned-") as tmp:
 root=pathlib.Path(tmp)
 def card(p,body):
  f=root/p;f.parent.mkdir(parents=True,exist_ok=True);f.write_text(body);return f
 a=card("a/Same.md","---\ntype: note\ndomain: x\n---\nFIRST CONTENT")
 b=card("b/Same.md","---\ntype: note\ndomain: x\n---\nDIFFERENT CONTENT")
 groups=common.collect_duplicate_groups(root,files=[a,b]);assert len(groups)==1 and a.read_bytes()!=b.read_bytes();results.append({"control":"same-stem-different-bytes","observed_groups":list(groups.values())})
 a=card("A.md","---\nemail: shared@example.test\n---\nA")
 b=card("B.md","---\nemail: shared@example.test\nphone: 12345\n---\nB")
 c=card("C.md","---\nphone: 12345\n---\nC")
 groups=common.collect_duplicate_groups(root,{"identity":{"max_shared":2}},files=[a,b,c]);assert len(groups)==1 and len(next(iter(groups.values())))==3;results.append({"control":"identity-bridge-component-exceeds-max-shared","observed_groups":list(groups.values()),"scope":"max_shared is per-key frequency guard, not documented component size limit; A/C share no strong field directly"})
 a=card("n/é.md","one");b=card("n/é.md","two")
 files=[a,b];idx=common.build_link_index(root,files);out=common.resolve_link_target("n/é",idx);assert out[1]=="exact" and len(idx["exact"])==1;results.append({"control":"NFC-path-key-coalescing","result":out,"scope":"Build index given explicit paths; filesystem may normalize aliases on macOS, so no distinct-on-disk files claimed"})
 f=card("repair.md","---\ndescription: "+("substantial repeated description unit "*4)+"\n---\nBODY\x00BYTES")
 before=f.read_bytes().split(b"---\n",2)[2];r=cleanup.clean_file(f,True);assert r["status"]=="cleaned" and f.read_bytes().split(b"---\n",2)[2]==before;results.append({"control":"cleanup-body-preservation-positive","result":r["status"]})
print(json.dumps({"author_controls":results,"count":len(results),"original_tests_run":False,"scope":"Unchanged common.py/cleanup.py, temporary owned cards; no user vault touched"},ensure_ascii=False,indent=2))
