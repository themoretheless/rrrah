#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,struct,shutil
import numpy as np
from PIL import Image,ImageOps
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-natural-large-gradient-banks-v2-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='complete_pending_audit' and len(d['extractions'])==2
assert all(sha(p)==h for p,h in d['pins'].items())and all(sha(p)==h for p,h in d['input_pins'].items())and all(sha(p)==h for p,h in d['dependency_pins'].items())
rows=[];kept=D/'dedup-natural-large-gradient-banks-v2-20261014';kept.mkdir(exist_ok=False)
for row in d['extractions']:
    assert row['returncode']==0
    for key in ['raw_pixels','features','stdout','stderr']:assert sha(row[key])==row[key+'_sha256']
    assert Path(row['stderr']).read_bytes()==b''
    w,h,total,n=map(int,Path(row['stdout']).read_text().split());assert 1536<n<=2048 and n==min(total,2048)and total<=3072
    raw=Path(row['raw_pixels']).read_bytes();assert struct.unpack('<II',raw[:8])==(w,h)and len(raw)==8+w*h*16
    with Image.open(row['input'])as source:
        assert bool(source.info.get('icc_profile'))==row['input_has_icc'];image=ImageOps.exif_transpose(source).convert('RGBA');assert image.size==(w,h);pixels=np.asarray(image,dtype=np.uint8)
    expected=pixels.astype(np.float32)/np.float32(255);rgb=expected[:,:,:3];expected[:,:,:3]=np.where(rgb<=0.04045,rgb/12.92,((rgb+0.055)/1.055)**2.4)
    actual=np.frombuffer(raw[8:],dtype='<f4').reshape(h,w,4)
    assert np.array_equal(actual,expected)and np.isfinite(actual).all()and np.all(actual[:,:,3]==1.)
    blob=Path(row['features']).read_bytes();assert struct.unpack('<Q',blob[:8])[0]==n and len(blob)==8+1040*n
    values=np.frombuffer(blob[8:],dtype='<f8').reshape(n,130);assert np.isfinite(values).all()and np.all(values[:,2:]>=0.)
    assert np.all(np.abs(np.sum(values[:,2:]**2,axis=1)-1.)<=1e-6)
    assert np.all(values[:,0]>=0.)and np.all(values[:,0]<w)and np.all(values[:,1]>=0.)and np.all(values[:,1]<h)
    for key in ['stdout','stderr']:
        p=Path(row[key]);target=kept/p.name;shutil.copyfile(p,target);assert sha(target)==sha(p)
    rows.append(dict(input=row['input'],dimensions=[w,h],extracted=total,retained=n,input_has_icc=row['input_has_icc'],unique_position_descriptor_records=len({bytes(v)for v in values}),features=row['features'],features_sha256=row['features_sha256']))
source=Path('/private/tmp/rrrah-natural-large-gradient-banks-v2-20261014/extract.rs');shutil.copyfile(source,kept/'extract.rs')
O=D/'dedup-natural-large-gradient-banks-v2-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_two_large_natural_primitive_banks',report=str(P.relative_to(R)),report_sha256=sha(P),auditor_sha256=sha(__file__),rows=rows,scope=d['scope'],limitations='Fixture preparation/descriptor validity, not native decoder pipeline or independent reimplementation of gradient extraction. Pixel preparation is unmanaged fixture work; no memory/RSS or corpus accuracy claim.'),indent=2)+'\n');print(O,[(x['extracted'],x['retained'])for x in rows])
