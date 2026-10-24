import pathlib,tempfile,json
from unittest.mock import patch
import enrich
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-enrich-owned-") as tmp:
 root=pathlib.Path(tmp)
 for folder in ['a','b']:
  (root/folder).mkdir();(root/folder/'Same.md').write_text('# Different '+folder)
 stems,mapping,_,_=enrich.scan_vault_for_links(root)
 assert len(stems)==1 and mapping['Same']=='b/Same.md'
 results.append({'control':'ambiguous-stem-last-path-wins','mapped_path':mapping['Same'],'scope':'Sorted walk, different files same stem; no semantic equivalence assumed'})
 out=root/'results';out.mkdir();files=[{'path':'a/Same.md','type':'note','domain':'x','tags':[],'existing_links':[],'summary':'owned'}]
 with patch('enrich.call_openrouter',return_value={'results':[{'path':'not-in-batch.md','links':['Same']}]}):
  data=enrich._process_swarm_link_batch('batch-001',files,'Same','x',{'Same'},'owned',0,out,True)
 assert data['results'][0]['path']=='not-in-batch.md' and data['results'][0]['matched_links']==['Same']
 results.append({'control':'response-file-not-checked-against-batch','accepted_path':'not-in-batch.md','scope':'API function mocked; no network or keys used'})
 p=root/'source.md';p.write_text('# Card\n## Related\n- old text\n## Later\n[[a/Same]]\n')
 (out/'batch-001-results.json').write_text(json.dumps({'results':[{'path':'source.md','matched_links':['Same']}]}))
 applied=enrich.apply_links(root,out,{'Same':'a/Same.md'})
 assert applied==0 and '[[a/Same]]' not in p.read_text().split('## Later')[0]
 results.append({'control':'later-section-link-suppresses-related-insertion','applied':applied,'scope':'Existing-link scan extends beyond Related section, actual link retained in Later'})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Original enrich/common, temporary owned notes and mocked model response; no user vault or external calls'},indent=2))
