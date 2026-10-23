import json
from hypoarena.text import tokenize, content_tokens
from hypoarena.dedup import DuplicateFinder, DedupConfig
cases=[]
for left,right,content in [('кошка','собака',False),('猫','犬',False),('protein binds receptor','protein does not bind receptor',True),('protein binds receptor','protein not binds receptor',True)]:
 config=DedupConfig(method='jaccard',threshold=1.0,shingle_unit='word',ngram_size=1,content_only=content)
 clusters=DuplicateFinder(config).find({'a':left,'b':right})
 cases.append(dict(left=left,right=right,content_only=content,left_tokens=list(tokenize(left)),right_tokens=list(tokenize(right)),left_content=list(content_tokens(left)),right_content=list(content_tokens(right)),clusters=[c.as_dict() for c in clusters]))
print(json.dumps({'scope':'Unmodified pinned package public API; four finite text cases, no file matching claim','cases':cases},ensure_ascii=False,indent=2))
