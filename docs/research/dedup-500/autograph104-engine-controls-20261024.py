import pathlib,tempfile,json,contextlib,io,engine
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-engine-owned-") as tmp:
 root=pathlib.Path(tmp);p=root/'a.md';p.write_text('---\ntype: note\ntier: core\nrelevance: 1.0\n---\nKeep')
 before=p.read_bytes()
 with contextlib.redirect_stdout(io.StringIO()):engine.cmd_decay(root,{})
 assert p.read_bytes()==before;results.append({'control':'core-tier-decay-preservation','bytes_unchanged':True})
 p.write_text('---\ntype: note\ntier: warm\nlast_accessed: 2000-01-01\n---\nBody')
 with contextlib.redirect_stdout(io.StringIO()):engine.cmd_decay(root,{})
 before=p.read_bytes()
 with contextlib.redirect_stdout(io.StringIO()):engine.cmd_decay(root,{})
 assert p.read_bytes()==before;results.append({'control':'same-day-repeat-decay-idempotence','bytes_unchanged_second_run':True})
 p.write_text('---\ntype: note\ntier: warm\nlast_accessed: 2999-01-01\n---\nBody')
 with contextlib.redirect_stdout(io.StringIO()):engine.cmd_decay(root,{})
 fm=engine.parse_frontmatter(p.read_text())[0];assert fm['relevance']>1
 results.append({'control':'future-date-relevance-unbounded-above-one','relevance':fm['relevance'],'scope':'No documented max score contract inferred; anomalous future date propagated'})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Unchanged engine/common with owned temporary files; no user vault'},indent=2))
