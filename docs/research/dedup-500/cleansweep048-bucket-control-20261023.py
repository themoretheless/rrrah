"""Independent mathematical check of the source's occupied-bucket neighbor rule.
Not an execution of Kotlin/Android code. All 256 first-byte buckets occupied.
"""
import json
keys=[f'{v:02x}' for v in range(256)]
results={}
for radius in [2,3,4,5,7,8]:
    qualifying=missed=0
    for a in range(256):
        for b in range(a+1,256):
            if (a^b).bit_count()<=radius:
                qualifying+=1
                missed+=abs(keys.index(f'{a:02x}')-keys.index(f'{b:02x}'))>1
    assert missed>0
    results[str(radius)]={'qualifying_pairs':qualifying,'omitted_by_neighbor_rule':missed}
example={'hash_a':'0000000000000000','hash_b':'8000000000000000','occupied_prefixes':['00','01','02','80'],'hamming_distance':1,'bucket_index_gap':3}
assert (int(example['hash_a'],16)^int(example['hash_b'],16)).bit_count()==1
print(json.dumps({'counterexample':example,'all_prefix_pairs':results},indent=2))
