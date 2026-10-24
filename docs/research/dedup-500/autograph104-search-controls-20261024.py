import pathlib,tempfile,json,search
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-search-owned-") as tmp:
 root=pathlib.Path(tmp);(root/'a.md').write_text('x'*8001+' uniqueendtoken')
 r=search.search_memory(root,'uniqueendtoken');assert r['count']==0
 results.append({'control':'body-tail-not-indexed','count':r['count'],'scope':'Body after first8000characters not searched'})
 (root/'templates').mkdir();(root/'templates/t.md').write_text('uniquetemplatetoken')
 r=search.search_memory(root,'uniquetemplatetoken',scope=['templates']);assert r['count']==0
 results.append({'control':'explicit-template-scope-still-ignored','count':r['count']})
 (root/'a.md').write_text('commonterm');(root/'b.md').write_text('commonterm')
 r=search.search_memory(root,'commonterm',limit=1);assert r['count']==2 and len(r['hits'])==1
 results.append({'control':'count-is-pre-limit-candidate-total','count':r['count'],'returned_hits':len(r['hits']),'scope':'API semantics observation, not inferred failure'})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Original search/common, owned temporary markdown, no user vault'},indent=2))
