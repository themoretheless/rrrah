"""Apply one isolated inference change to the pinned loftr 0.1.1 source copy."""
from pathlib import Path
import sys

root=Path(sys.argv[1])
p=root/'src/loftr_model.rs'
s=p.read_text()
old='self.forward_with_debug(image0, image1)?'
assert s.count(old)==2
s=s.replace(old,'self.forward_impl(image0, image1, false)?',1)
s=s.replace(old,'self.forward_impl(image0, image1, true)?',1)
s=s.replace('Ok(stages.debug)','stages.debug.ok_or_else(|| LoftrError::InvalidConfig("missing requested debug statistics".into()))')
old='''    fn forward_with_debug(
        &mut self,
        image0: &Tensor,
        image1: &Tensor,
    ) -> Result<ForwardWithDebug, LoftrError> {'''
new=old.replace('forward_with_debug','forward_impl').replace('        image1: &Tensor,','        image1: &Tensor,\n        collect_debug: bool,').replace('ForwardWithDebug','ForwardOutput')
assert s.count(old)==1
s=s.replace(old,new)
old='''            debug: self.build_debug_stages(
                &image0,
                &image1,
                &backbone_features,
                &coarse_features,
                &coarse,
            ),'''
new=old.replace('debug: self.build_debug_stages(','debug: if collect_debug { Some(self.build_debug_stages(').replace('            ),','            )) } else { None },')
assert s.count(old)==1
s=s.replace(old,new).replace('ForwardWithDebug','ForwardOutput').replace('    debug: LoftrDebugStages,','    debug: Option<LoftrDebugStages>,')
p.write_text(s)
p=root/'src/loftr_model/tests.rs'
with p.open('a') as f:
    f.write('''
#[test]
fn regression_no_eager_debug_preserves_outputs_and_public_debug() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    tch::manual_seed(173);
    let mut model = LoFTRModel::new(Device::Cpu, LoftrConfig::outdoor())?;
    let image0 = Tensor::rand([1, 1, 64, 64], (Kind::Float, Device::Cpu));
    let image1 = Tensor::rand([1, 1, 64, 64], (Kind::Float, Device::Cpu));
    let _inference = tch::no_grad_guard();
    let plain = model.forward_impl(&image0, &image1, false)?;
    let debug = model.forward_impl(&image0, &image1, true)?;
    assert!(plain.debug.is_none());
    assert!(debug.debug.is_some());
    assert!(plain.matches.keypoints0.equal(&debug.matches.keypoints0));
    assert!(plain.matches.keypoints1.equal(&debug.matches.keypoints1));
    assert!(plain.matches.confidence.equal(&debug.matches.confidence));
    assert!(plain.matches.batch_indexes.equal(&debug.matches.batch_indexes));
    let public = model.forward_debug(&image0, &image1)?;
    assert_eq!(public.image0.shape, vec![1, 1, 64, 64]);
    Ok(())
}
''')
