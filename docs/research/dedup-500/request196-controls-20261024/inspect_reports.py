from pathlib import Path
import json,math,statistics,collections,hashlib,xml.etree.ElementTree as ET
from html.parser import HTMLParser
from urllib.parse import urlparse,unquote
b=Path(__file__).resolve().parent;e=b.parent/'evidence/garypen--deduplicate';root=e/'remaining-pinned';acq=json.loads((b/'reports-acquisition.json').read_text());assert acq['verified']==acq['expected']==592
rows=[];counts=collections.Counter();issues=[];comparisons=[];sample_rows=0;svg_texts=collections.Counter();html_links=0;local_missing=[]
class HTML(HTMLParser):
 def __init__(self):super().__init__(convert_charrefs=True);self.links=[];self.text=[];self.tagcounts=collections.Counter()
 def handle_starttag(self,tag,attrs):
  self.tagcounts[tag]+=1
  for k,v in attrs:
   if k in ['href','src','data'] and v:self.links.append((tag,k,v))
 def handle_data(self,data):self.text.append(data)
def numbers(x):
 if isinstance(x,dict):
  for v in x.values():yield from numbers(v)
 elif isinstance(x,list):
  for v in x:yield from numbers(v)
 elif isinstance(x,(int,float)) and not isinstance(x,bool):yield x
for record in acq['artifacts']:
 path=record['path'];f=root/path;raw=f.read_bytes();assert hashlib.sha256(raw).hexdigest()==record['sha256'];s=raw.decode('utf8');suffix=f.suffix;counts[suffix]+=1;row={'path':path,'sha256':record['sha256']}
 if suffix=='.json':
  data=json.loads(s,parse_constant=lambda v:(_ for _ in ()).throw(ValueError(v)));ns=list(numbers(data));assert all(math.isfinite(v) for v in ns);row.update(kind='full-structured-JSON',numeric_values=len(ns))
  if f.name=='sample.json':
   its,ts=data['iters'],data['times'];assert len(its)==len(ts)>0 and all(v>0 for v in its) and all(v>=0 for v in ts);sample_rows+=len(its);norm=[t/n for t,n in zip(ts,its)];est=json.loads((f.parent/'estimates.json').read_text());comp={'sample':path,'samples':len(norm),'sampling_mode':data['sampling_mode']}
   for name,value in [('mean',statistics.mean(norm)),('median',statistics.median(norm))]:
    published=est[name]['point_estimate'];delta=abs(value-published);close=math.isclose(value,published,rel_tol=1e-10,abs_tol=1e-7);comp[name]={'recomputed':value,'published':published,'absolute_difference':delta,'matches':close}
    if not close:issues.append({'kind':'recomputed-estimate-mismatch','path':path,'metric':name,'absolute_difference':delta})
   comparisons.append(comp)
 elif suffix=='.html':
  parser=HTML();parser.feed(s);parser.close();html_links+=len(parser.links);row.update(kind='complete-HTML-structure-and-text-extraction-not-rendering',text_characters=sum(map(len,parser.text)),tags=dict(parser.tagcounts),links=len(parser.links))
  for tag,attr,url in parser.links:
   parsed=urlparse(url)
   if not parsed.scheme and not parsed.netloc and parsed.path:
    target=(f.parent/unquote(parsed.path)).resolve()
    if not target.exists():local_missing.append({'from':path,'attribute':attr,'url':url})
 elif suffix=='.svg':
  doc=ET.fromstring(s);tags=collections.Counter(n.tag.split('}')[-1] for n in doc.iter());texts=[''.join(n.itertext()) for n in doc.iter() if n.tag.split('}')[-1] in ['text','title','desc']];row.update(kind='complete-SVG-XML-traversal-not-visual-viewing',elements=sum(tags.values()),tags=dict(tags),text_records=len(texts),viewBox=doc.attrib.get('viewBox'));svg_texts.update(texts)
 else:raise ValueError(path)
 rows.append(row)
p={'scope':'All592 pinned generatedCriterion artifacts acquired/hashverified and fully machine-traversed.258JSON numeric records inspected;31HTML parsed/text extracted;303SVG structurally traversed,NOT visually viewed. No script execution/browser render/performance run.','counts':dict(counts),'sample_files':len(comparisons),'sample_pairs_traversed':sample_rows,'estimate_comparisons':comparisons,'issues':issues,'html_links':html_links,'missing_local_links':local_missing,'svg_text_records_total':sum(svg_texts.values()),'new_visual_views':0,'svg_visual_remaining':303,'artifact_records':rows,'limits':['Recomputes arithmetic mean/median from storednormalizedsamples only;doesnotrecompute bootstrapCIs/standarderrors,slope/outlierclassification or producer provenance.','Historical0.3.2/0.3.6/base/new comparisons are not fresh0.4.1 benchmark or imagefileworkload qualification.','ParsedHTML/SVG and countcoverage do not constitute visualreview.']};(b/'reports-structured-results.json').write_text(json.dumps(p,indent=2)+'\n');print(json.dumps({k:p[k] for k in ['counts','sample_files','sample_pairs_traversed','issues','html_links','svg_text_records_total','svg_visual_remaining']},indent=2));print('missing local links',len(local_missing))
