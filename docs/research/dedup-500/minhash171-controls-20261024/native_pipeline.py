"""Complete unchanged original five-stage pipeline, actual local fsspec IO.

Owned text documents only. No stage/algorithm adapters or optional analysis;
not a large-corpus, image/file-byte equality, S3 or scheduler qualification.
"""
import importlib.metadata
import json
from pathlib import Path
import sys

sys.path.insert(0,str(Path(__file__).with_name('runtime')))
from minhash_dedup import SignatureStage,BucketStage,PruneStage,SelectionStage,FilterStage,MinhashConfig,HashConfig
from minhash_dedup.runtime import Document

root=Path(__file__).with_name('owned-native-pipeline');root.mkdir(exist_ok=True)
base='the quick brown fox owns this sample text '*4
texts=[base,base,base.replace('o','o!'),'ZZYX distinct alpha beta gamma data '*4,'hi']
docs=[Document(text=text,id=str(i)) for i,text in enumerate(texts)]
config=MinhashConfig(n_grams=3,num_buckets=2,hashes_per_bucket=2,hash_config=HashConfig(precision=64,hash_fc='sha1'))
signatures=root/'signatures';buckets=root/'buckets';pruned=root/'pruned';selected=root/'selected'
SignatureStage(signatures,config).run(docs)
for rank in range(2):BucketStage(signatures,buckets,config).run(rank=rank,world_size=2)
for rank in range(2):PruneStage(buckets,pruned).run(rank=rank,world_size=2)
SelectionStage(signatures,pruned,selected,config=config).run()
retained=list(FilterStage(selected).run(docs))
assert [doc.id for doc in retained]==['0','3','4']
assert retained[0].metadata['dup_signals']['dup_doc_count']==2
result={'scope':__doc__,'retained_ids':[doc.id for doc in retained],'normalized_duplicate_ids':['1','2'],'short_no_shingles_retained':retained[-1].id=='4','duplicate_count':retained[0].metadata['dup_signals']['dup_doc_count'],'authored_controls_run':1,'original_tests_run':0,'runtime_versions':{p:importlib.metadata.version(p) for p in ['numpy','fsspec','nltk','loguru','orjson','tqdm','xxhash']}}
Path(__file__).with_name('native-pipeline-results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
