# Independent ASCII model of pinned qElide, not execution of Qt source.
import json
prefix="/root/"+"p"*12
suffix="s"*18
paths=[prefix+"/alpha-middle/"+suffix,prefix+"/bravo-middle/"+suffix]
def elide(value, maximum=40):
    half=(maximum-3)//2
    return value[:half]+"..."+value[-half:] if len(value)>maximum else value
labels=[elide(path) for path in paths]
assert paths[0]!=paths[1] and labels[0]==labels[1]
print(json.dumps({"paths":paths,"labels":labels,"distinct_paths":2,"distinct_display_keys":len(set(labels))},indent=2))
