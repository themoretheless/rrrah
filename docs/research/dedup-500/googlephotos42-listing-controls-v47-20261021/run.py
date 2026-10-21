import json, re, hashlib
from pathlib import Path
base=Path('.dedup-work/googlephotos42-20261021/storefront')
pack=(base/'PASTE-PACK.md').read_text()
data=json.loads((base/'listing.json').read_text())
matched=[]; novel=[]
def walk(v,key=''):
    if isinstance(v,dict):
        for k,x in v.items():walk(x,key+'/'+k)
    elif isinstance(v,list):
        for i,x in enumerate(v):walk(x,key+'/'+str(i))
    elif isinstance(v,str) and v in pack:matched.append(key)
    else:novel.append({'path':key,'value':v})
walk(data)
assert len(matched)==200 and len(novel)==8
print('PASS all208 parsed leaves classified:200 exact substrings of fully-read pack,8 novel values independently read')
sections=re.split(r'^### ',pack,flags=re.M)
results={}
for code in data['cws']['localized']:
    section=next(s for s in sections[1:] if s.splitlines()[0].endswith('('+code+')'))
    section=section.split('\n## ',1)[0]
    blocks=re.findall(r'```text\n(.*?)\n```',section,re.S)
    assert len(blocks)==3
    obj=data['cws']['localized'][code]
    assert blocks[0]==obj['title'] and blocks[1]==obj['summary']
    expected='\n\n'.join(obj.get('description',data['cws']['description']))
    results[code]={'title_summary_equal':True,'description_equal':blocks[2]==expected}
assert [k for k,v in results.items() if not v['description_equal']]==['ja']
print('PASS all10 CWS title/summary pairs identical;9 descriptions identical;Japanese description differs (9 vs10 UI languages)')
edge=pack.split('## 2. Edge Add-ons: first submission',1)[1].split('## 3. Firefox AMO',1)[0]
edgeblocks=re.findall(r'```text\n(.*?)\n```',edge,re.S)
assert len(edgeblocks)==2 and edgeblocks[0]==data['edge']['shortDescription']
assert edgeblocks[1]!='\n\n'.join(data['edge']['description'])
print('PASS Edge short description identical but long description differs')
print(json.dumps({'matched_leaf_count':len(matched),'novel_leaves':novel,'cws':results},ensure_ascii=False,indent=2))
