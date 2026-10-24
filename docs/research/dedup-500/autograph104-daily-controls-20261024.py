import tempfile,pathlib,json,daily
results=[]
e=daily.extract_entities('@alice @alice',{});rels=daily.build_relationships(e)
assert len(e['people'])==2 and len(rels)==1 and rels[0]['from']==rels[0]['to']=='@alice'
results.append({'control':'repeated-mention-self-relationship','people_count':2,'relationship':rels[0]})
e=daily.extract_entities('Contact person@example.test',{});assert any(x.get('handle')=='@example' for x in e['people'])
results.append({'control':'email-domain-read-as-mention','people':e['people']})
e=daily.extract_entities('```\n@sampleuser $123 approved example\n```',{});assert e['people'] and e['financial'] and e['decisions'] and not e['wikilinks']
results.append({'control':'code-examples-parsed-as-facts','people':len(e['people']),'financial':len(e['financial']),'decisions':len(e['decisions']),'scope':'Mention/amount/decision extraction uses raw text; wikilink extractor separately ignores code'})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Original daily/common functions on literal owned strings; no vault mutation'},indent=2))
