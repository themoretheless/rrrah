#!/usr/bin/env python3
"""Natural descriptor-bank benchmark fixtures; Pillow prep is explicit, not native decode proof."""
from pathlib import Path
import hashlib,json,os,struct,subprocess
from PIL import Image,ImageOps
import numpy as np
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
natural_path=D/'dedup-gradient-unroll4-natural-cpu-20261014.json';natural=json.loads(natural_path.read_text());inputs=natural['cases'][2]['inputs']
old_plan=D/'dedup-gradient-natural-bound-benchmark-plan-20261013.json';old=json.loads(old_plan.read_text())
# Case2 is table_true; resolve its actual original images, not feature dumps.
images=natural['cases'][2]['source_images'];assert len(images)==2
assert all(sha(p)==old['pins'][p]for p in images)
prior_path=D/'dedup-gradient-unroll4-managed-benchmark-20261014.json';prior=json.loads(prior_path.read_text());assert prior['status']=='complete_pending_audit'
library=Path('/private/tmp/rrrah-gradient-unroll4-managed-benchmark-20261014/candidate/librrrah_dedup.rlib');assert sha(library)==prior['pins'][str(library)]
F=Path('/private/tmp/rrrah-natural-large-gradient-banks-20261014');F.mkdir(exist_ok=False)
src=F/'extract.rs';src.write_text(r'''use std::{io::Write,path::Path};
use rrrah_dedup::{linear::LinearRgbaView,local::LocalPolicy,pyramid::PyramidPolicy,gradient::{extract_gradient_pyramid_with_recipe,GradientCellRecipe}};
fn main(){let args:Vec<_>=std::env::args().skip(1).collect();assert_eq!(args.len(),2);let bytes=std::fs::read(&args[0]).unwrap();let w=u32::from_le_bytes(bytes[..4].try_into().unwrap());let h=u32::from_le_bytes(bytes[4..8].try_into().unwrap());let count=u64::from(w)*u64::from(h);assert!(count<=12800000);assert_eq!(bytes.len(),8+count as usize*16);let pixels:Vec<[f32;4]>=bytes[8..].chunks_exact(16).map(|b|std::array::from_fn(|i|f32::from_le_bytes(b[i*4..i*4+4].try_into().unwrap()))).collect();let view=LinearRgbaView::new(w,h,&pixels,12800000,||false).unwrap();let local=LocalPolicy{max_pixels:12800000,max_candidates:12800000,max_features:1024,minimum_corner_score:0.0001};let policy=PyramidPolicy{local,max_levels:3,max_total_pixels:12800000,max_total_features:3072};let mut bank=extract_gradient_pyramid_with_recipe(&view,policy,3072*512,GradientCellRecipe::Interpolated,||false).unwrap();let extracted=bank.len();bank.truncate(2048);let mut output=std::fs::OpenOptions::new().write(true).create_new(true).open(Path::new(&args[1])).unwrap();output.write_all(&(bank.len()as u64).to_le_bytes()).unwrap();for f in &bank{for v in f.position.iter().chain(&f.descriptor.0){output.write_all(&v.to_bits().to_le_bytes()).unwrap();}}println!("{} {} {} {}",w,h,extracted,bank.len());}
''')
control_path=D/'dedup-gradient-unroll4-managed-focused-v2-20261014.json';control=json.loads(control_path.read_text());assert all(sha(p)==h for p,h in control['dependency_pins'].items())
deps=next(v.split('=',1)[1]for k,v in zip(control['command'],control['command'][1:])if k=='-L')
exe=F/'extract';command=['rustc','+stable','--edition=2024','-C','opt-level=3',str(src),'--extern','rrrah_dedup='+str(library),'-L','dependency='+deps,'-o',str(exe)]
O=D/'dedup-natural-large-gradient-banks-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),pins={str(p):sha(p)for p in [Path(__file__),natural_path,old_plan,prior_path,control_path,src,library]},input_pins={p:sha(p)for p in images},dependency_pins=control['dependency_pins'],command=command,extractions=[],scope='Two retained public table photographs, complete source hashes verified. Pillow EXIFtranspose/RGBA8 interpreted as sRGB, standard inverse transfer float32linear opaque pixels, native currentV7Rust gradient pyramid1024perlevel/3levels, first2048actual features; no synthetic repetition/padding. Primitive matcher fixture only, not native decoder/ICC/default/corpus/managed memory/RSS qualification.')
def save():
    temp=O.with_suffix('.tmp');temp.write_text(json.dumps(state,indent=2)+'\n');temp.replace(O)
save()
with (F/'build.log').open('x')as log:
    p=subprocess.Popen(command,stdout=log,stderr=subprocess.STDOUT);state['child_pid']=p.pid;save();code=p.wait()
assert code==0,(F/'build.log').read_text()[-5000:]
state['pins'][str(exe)]=sha(exe)
for index,path in enumerate(images):
    with Image.open(path)as original:
        icc=bool(original.info.get('icc_profile'));image=ImageOps.exif_transpose(original).convert('RGBA');width,height=image.size;assert width*height<=12800000
        rgba=np.asarray(image,dtype=np.uint8);assert np.all(rgba[:,:,3]==255)
    normalized=rgba.astype(np.float32)/np.float32(255);rgb=normalized[:,:,:3];normalized[:,:,:3]=np.where(rgb<=0.04045,rgb/12.92,((rgb+0.055)/1.055)**2.4)
    raw=F/f'pixels-{index}.bin';raw.write_bytes(struct.pack('<II',width,height)+normalized.astype('<f4').tobytes())
    output=F/f'features-{index}.bin';stdout=F/f'extract-{index}.stdout';stderr=F/f'extract-{index}.stderr'
    with stdout.open('x')as out,stderr.open('x')as err:
        p=subprocess.Popen([str(exe),str(raw),str(output)],stdout=out,stderr=err);state.update(status='extracting',active_index=index,child_pid=p.pid);save();code=p.wait()
    state['extractions'].append(dict(input=path,input_has_icc=icc,raw_pixels=str(raw),raw_pixels_sha256=sha(raw),features=str(output),features_sha256=sha(output)if output.exists()else None,returncode=code,stdout=str(stdout),stdout_sha256=sha(stdout),stderr=str(stderr),stderr_sha256=sha(stderr)));save()
    assert code==0,stderr.read_text()
    w,h,total,retained=map(int,stdout.read_text().split());assert (w,h)==(width,height)and retained==min(total,2048)and output.stat().st_size==8+retained*1040
    assert retained>1536,('Not a large natural bank; no padding allowed',retained)
assert all(sha(p)==h for p,h in state['pins'].items()) and all(sha(p)==h for p,h in state['input_pins'].items())
state['status']='complete_pending_audit';save();print(O)
