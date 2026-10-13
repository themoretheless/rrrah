#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-exact-lossy-path-source-20261012.json';m=json.loads(parent.read_text());s=T/'rrrah-loftr-selected-windows-candidate-20261012';assert not s.exists()
 for k,h in m['file_hashes'].items():
  p=Path(m['snapshot'])/k;assert sha(p)==h
  if k.startswith(('crates/rrrah-dedup/src/','vendor/loftr/src/')):assert sha(R/k)==h,k
  dest=s/k;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,dest)
 p=s/'vendor/loftr/src/fine_preprocess.rs';code=p.read_text()
 old='''        let fine_windows0 = unfold_local_windows(fine_map0, self.window_size, stride)?;
        let fine_windows1 = unfold_local_windows(fine_map1, self.window_size, stride)?;

        let fine_windows0 = select_unfold_windows(&fine_windows0, &data.b_ids, &data.i_ids)?;
        let fine_windows1 = select_unfold_windows(&fine_windows1, &data.b_ids, &data.j_ids)?;'''
 new='''        let (fine_windows0, fine_windows1) = if selected_windows_eligible(fine_map0, &data.b_ids, &data.i_ids, stride)
            && selected_windows_eligible(fine_map1, &data.b_ids, &data.j_ids, stride)
        {
            (gather_selected_windows(fine_map0, &data.i_ids, self.window_size, stride)?,
             gather_selected_windows(fine_map1, &data.j_ids, self.window_size, stride)?)
        } else {
            let fine_windows0 = unfold_local_windows(fine_map0, self.window_size, stride)?;
            let fine_windows1 = unfold_local_windows(fine_map1, self.window_size, stride)?;
            (select_unfold_windows(&fine_windows0, &data.b_ids, &data.i_ids)?,
             select_unfold_windows(&fine_windows1, &data.b_ids, &data.j_ids)?)
        };'''
 assert code.count(old)==1
 # Keep the complete old forward implementation solely as an independent test oracle.
 start=code.index('    pub fn forward(');end=code.index('\n}\n\nfn first_dim',start)
 reference=code[start:end].replace('    pub fn forward(', '    #[cfg(test)]\n    fn forward_dense_reference(',1)
 code=code.replace(old,new)
 end=code.index('\n}\n\nfn first_dim');code=code[:end]+'\n'+reference+code[end:]
 helper='''
// CPU single-pair inference can collect just the selected windows. Other batches,
// devices, dtypes and invalid index cases retain the original im2col path.
fn selected_windows_eligible(feat: &Tensor, b_ids: &Tensor, tokens: &Tensor, stride: i64) -> bool {
    let d = feat.size();
    if d.len()!=4 || d[0]!=1 || d[2]<=0 || d[3]<=0 || stride<=0
        || feat.device()!=Device::Cpu || feat.kind()!=Kind::Float || tokens.numel()==0 {
        return false;
    }
    let grid_h = 1 + (d[2]-1)/stride;
    let grid_w = 1 + (d[3]-1)/stride;
    let Some(count) = grid_h.checked_mul(grid_w) else { return false; };
    let Ok(b) = normalize_index_tensor(b_ids, Device::Cpu) else { return false; };
    let Ok(t) = normalize_index_tensor(tokens, Device::Cpu) else { return false; };
    b.eq(0).all().int64_value(&[])!=0 && t.min().int64_value(&[])>=0 && t.max().int64_value(&[])<count
}

fn gather_selected_windows(feat: &Tensor, tokens: &Tensor, window: i64, stride: i64) -> Result<Tensor,LoftrError> {
    let d=feat.size();let channels=d[1];let height=d[2];let width=d[3];let grid_w=1+(width-1)/stride;
    let tokens=normalize_index_tensor(tokens,Device::Cpu)?;
    let offsets=Tensor::arange(window,(Kind::Int64,Device::Cpu))-window/2;
    let dy=offsets.unsqueeze(1).repeat([1,window]).reshape([1,-1]);
    let dx=offsets.unsqueeze(0).repeat([window,1]).reshape([1,-1]);
    let y=tokens.floor_divide_scalar(grid_w).unsqueeze(1)*stride+dy;
    let x=tokens.remainder(grid_w).unsqueeze(1)*stride+dx;
    let valid=y.ge(0).logical_and(&y.lt(height)).logical_and(&x.ge(0)).logical_and(&x.lt(width));
    let ids=(y.clamp(0,height-1)*width+x.clamp(0,width-1)).reshape([-1]);
    let selected=feat.reshape([channels,height*width]).index_select(1,&ids);
    let selected=selected.masked_fill(&valid.reshape([1,-1]).logical_not(),0.0);
    Ok(selected.reshape([channels,tokens.size()[0],window*window]).permute([1,2,0]).contiguous())
}
'''
 code=code.replace('\nfn first_dim(',helper+'\nfn first_dim(',1);p.write_text(code)
 tests=s/'vendor/loftr/src/fine_preprocess/tests.rs'
 tests.write_text(tests.read_text()+'''

#[test]
fn selected_windows_match_complete_dense_forward_with_borders_repeats_and_fallbacks() -> Result<(),LoftrError> {
    tch::set_num_threads(2);
    let _inference=tch::no_grad_guard();
    for (batch,height,width,stride,window,concat,noncontiguous) in [
        (1,8,12,2,5,false,false),(1,9,13,4,3,true,false),
        (1,1,1,1,7,false,false),(1,8,12,2,1,true,false),
        (2,8,12,2,5,false,false),(1,12,8,2,5,true,true),
    ] {
        let mut config=LoftrConfig::outdoor();config.fine_window_size=window;config.fine_concat_coarse_feat=concat;
        let module=cpu_module(&config)?;
        let (height,width)=if noncontiguous {(width,height)} else {(height,width)};
        let map=Tensor::arange(batch*128*height*width,(Kind::Float,Device::Cpu)).reshape([batch,128,height,width]);
        let map=if noncontiguous {map.transpose(2,3)} else {map};
        let h=map.size()[2];let w=map.size()[3];let gh=1+(h-1)/stride;let gw=1+(w-1)/stride;
        let last=gh*gw-1;
        let data=FinePreprocessData{hw0_f:(gh*stride,gw*stride),hw0_c:(gh,gw),b_ids:Tensor::from_slice(&[0_i64,batch-1,0,0]),i_ids:Tensor::from_slice(&[0_i64,last,0,last/2]),j_ids:Tensor::from_slice(&[last,0_i64,last,0])};
        let other=&map+4096.0;
        let tokens=Tensor::arange(batch*gh*gw*256,(Kind::Float,Device::Cpu)).reshape([batch,gh*gw,256])/1024.0;
        let actual=module.forward(&map,&other,&tokens,&tokens,&data)?;
        let expected=module.forward_dense_reference(&map,&other,&tokens,&tokens,&data)?;
        assert!(actual.0.equal(&expected.0),"left batch={batch} shape={h}x{w} window={window} concat={concat}");
        assert!(actual.1.equal(&expected.1),"right batch={batch} shape={h}x{w} window={window} concat={concat}");
        assert!(actual.0.is_contiguous());assert!(actual.1.is_contiguous());
    }
    Ok(())
}

#[test]
fn selected_window_gradient_matches_dense_repeated_border_windows() -> Result<(),LoftrError> {
    tch::set_num_threads(2);
    let mut config=LoftrConfig::outdoor();config.fine_concat_coarse_feat=false;
    let module=cpu_module(&config)?;
    let data=FinePreprocessData{hw0_f:(8,12),hw0_c:(4,6),b_ids:Tensor::from_slice(&[0_i64,0,0]),i_ids:Tensor::from_slice(&[0_i64,23,0]),j_ids:Tensor::from_slice(&[23_i64,0,23])};
    let tokens=Tensor::zeros([1,24,256],(Kind::Float,Device::Cpu));
    let make=||Tensor::arange(128*8*12,(Kind::Float,Device::Cpu)).reshape([1,128,8,12]).set_requires_grad(true);
    let a=make();let b=make();let da=make();let db=make();
    let x=module.forward(&a,&b,&tokens,&tokens,&data)?;let y=module.forward_dense_reference(&da,&db,&tokens,&tokens,&data)?;
    (x.0.sum(Kind::Float)+x.1.sum(Kind::Float)).backward();(y.0.sum(Kind::Float)+y.1.sum(Kind::Float)).backward();
    assert!(a.grad().equal(&da.grad()));assert!(b.grad().equal(&db.grad()));Ok(())
}

#[test]
#[ignore="manual fresh-process A/B selected-window RSS control"]
fn selected_windows_memory_control() -> Result<(),LoftrError> {
    tch::set_num_threads(2);let _inference=tch::no_grad_guard();
    let mut config=LoftrConfig::outdoor();config.fine_concat_coarse_feat=false;
    let module=cpu_module(&config)?;let side=320_i64;let count=256_i64;
    let map=Tensor::arange(128*side*side,(Kind::Float,Device::Cpu)).reshape([1,128,side,side]);let other=&map+1024.0;
    let tokens=Tensor::zeros([1,80*80,256],(Kind::Float,Device::Cpu));
    let data=FinePreprocessData{hw0_f:(side,side),hw0_c:(80,80),b_ids:Tensor::zeros([count],(Kind::Int64,Device::Cpu)),i_ids:Tensor::arange(count,(Kind::Int64,Device::Cpu))*23,j_ids:Tensor::arange(count,(Kind::Int64,Device::Cpu))*19};
    let mode=std::env::var("RRRAH_SELECTED_WINDOW_CONTROL").unwrap();
    let out=match mode.as_str(){"dense"=>module.forward_dense_reference(&map,&other,&tokens,&tokens,&data)?,"selected"=>module.forward(&map,&other,&tokens,&tokens,&data)?,_=>panic!("invalid mode")};
    let mut hash=14695981039346656037_u64;
    for tensor in [&out.0,&out.1] {for value in Vec::<f32>::try_from(tensor.reshape([-1]))? {for byte in value.to_bits().to_le_bytes(){hash=(hash^u64::from(byte)).wrapping_mul(1099511628211);}}}
    println!("selected_window_control mode={mode} hash={hash:016x} shape={:?}",out.0.size());Ok(())
}
''')
 hashes={k:sha(s/k) for k in m['file_hashes']};out=D/'dedup-loftr-selected-windows-source-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'isolated_selected_window_candidate_not_adopted','snapshot':str(s),'parent_manifest':str(parent),'parent_sha256':sha(parent),'file_hashes':hashes,'deltas_from_parent':[k for k in hashes if hashes[k]!=m['file_hashes'][k]],'producer_sha256':sha(Path(__file__)),'scope':'Single-batch CPU float selected-window gather; original dense path retained for other batches/devices/dtypes and invalid indexes. Full original forward retained only as cfg(test) oracle. Not yet qualified or adopted.'},indent=2)+'\n');print(s)
if __name__=='__main__':main()
