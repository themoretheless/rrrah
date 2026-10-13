from pathlib import Path
import shutil,hashlib,json
R=Path('/Users/themoretheless/Documents/Sources/rrrah');T=Path('/Users/themoretheless/.codex/tmp');H=T/'rrrah-loftr-position-floor-units-20261012'
assert not H.exists()
shutil.copytree(R/'vendor/loftr/src',H/'src')
for n in ['Cargo.toml','Cargo.lock']:shutil.copy2(T/'rrrah-loftr-sequential-current-units-20261012'/n,H/n)
p=H/'src/loftr_model.rs';s=p.read_text();assert s.count('(256, 256),')==1;p.write_text(s.replace('(256, 256),','(1, 1),'))
p=H/'src/position_encoding/tests.rs'
p.write_text(p.read_text()+'''
#[test]
fn small_floor_matches_original_floor_for_reused_inputs() -> Result<(), LoftrError> {
    for fixed in [false, true] {
        let mut original = PositionEncodingSine::new(256, (256, 256), fixed, Device::Cpu)?;
        let mut candidate = PositionEncodingSine::new(256, (1, 1), fixed, Device::Cpu)?;
        for (height, width) in [(80, 80), (48, 80), (80, 48), (1, 1), (257, 2), (2, 257), (80, 80)] {
            let input = Tensor::randn([1, 256, height, width], (Kind::Float, Device::Cpu));
            let expected = original.forward(&input)?;
            let actual = candidate.forward(&input)?;
            let delta = (&actual - &expected).abs().max().double_value(&[]);
            assert_eq!(delta, 0., "fixed={fixed}, shape={height}x{width}");
            assert_eq!(candidate.pe.size(), vec![1, 256, height, width]);
        }
    }
    Ok(())
}
''')
files={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in H.rglob('*') if p.is_file()}
(R/'docs/research/dedup-loftr-position-floor-candidate-20261012.json').write_text(json.dumps(dict(status='prepared_isolated_not_adopted',source=str(H),pins=files,production_delta=['loftr_model.rs'],test_delta=['position_encoding/tests.rs'],scope='Candidate only. Exact equality and full process RSS qualification required before adoption.'),indent=2)+'\n')
print(H)
