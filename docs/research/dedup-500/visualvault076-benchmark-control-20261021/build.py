from pathlib import Path
import subprocess
root = Path.cwd()
old = Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-realmodel-20261021')
dest = Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-benchmark-20261021')
(dest / 'src').mkdir(parents=True, exist_ok=True)
s = (old / 'src/lib.rs').read_text() + '''
#[tokio::test]
async fn control_detection_future_does_not_capture_hashes_until_polled() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a"); let b = root.path().join("b");
    std::fs::write(&a, b"same").unwrap(); std::fs::write(&b, b"same").unwrap();
    let files = [media(a.clone(), 4), media(b.clone(), 4)];
    let detector = detector::DuplicateDetector::new();
    assert_eq!(detector.detect_duplicates(&files, false).await.unwrap().total_groups, 1);
    let future = detector.detect_duplicates(&files, false);
    std::fs::remove_file(a).unwrap(); std::fs::remove_file(b).unwrap();
    assert_eq!(future.await.unwrap().total_groups, 0);
}
'''
(dest / 'src/lib.rs').write_text(s)
(dest / 'Cargo.toml').write_text((old / 'Cargo.toml').read_text().replace('visualvault076-realmodel', 'visualvault076-benchmark'))
log = root / 'docs/research/dedup-500/visualvault076-benchmark-control-20261021/run.log'
with log.open('w') as out:
    subprocess.run(['cargo', '+1.98.1', 'test', '--manifest-path', str(dest / 'Cargo.toml'), '--target-dir', str(root / '.dedup-work/visualvault076-detector-target-20261021'), 'control_detection_future_does_not_capture_hashes_until_polled'], stdout=out, stderr=subprocess.STDOUT, check=True)
log.write_text(log.read_text().rstrip() + '\n')
