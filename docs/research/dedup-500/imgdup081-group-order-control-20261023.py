#!/usr/bin/env python3
"""Run the original manage_images body with explicit synthetic image/hash adapters."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

upstream, work = map(Path, sys.argv[1:])
source = (upstream / "src/processing.rs").read_bytes()
assert hashlib.sha1(b"blob " + str(len(source)).encode() + b"\0" + source).hexdigest() == "".join(
    x["sha"] for x in json.loads((Path(__file__).parent / "evidence/abonander--img-dup/tree.json").read_text())["tree"]
    if x["path"] == "src/processing.rs"
)
text = source.decode()
start = text.index("pub fn manage_images(")
opening = text.index("{", start)
depth = 1
end = opening + 1
while depth:
    depth += (text[end] == "{") - (text[end] == "}")
    end += 1
body = text[start:end]
work.mkdir(parents=True, exist_ok=True)
prefix = r'''
#[derive(Clone)]
pub struct Image { id: u8, bits: u8 }
pub struct UniqueImage { img: Image, similars: Vec<Image> }
pub struct ProgramSettings { threshold: f32 }
impl UniqueImage {
    fn from_image(img: Image) -> Self { Self { img, similars: vec![] } }
    fn is_similar(&self, other: &Image, threshold: f32) -> bool {
        (self.img.bits ^ other.bits).count_ones() as f32 / 4.0 < threshold
    }
    fn add_similar(&mut self, img: Image) { self.similars.push(img); }
}
'''
suffix = r'''
fn main() {
    let permutations = [[0,1,2], [0,2,1], [1,0,2], [1,2,0], [2,0,1], [2,1,0]];
    let bits = [0b0000, 0b0011, 0b1111];
    let policy = ProgramSettings { threshold: 0.6 };
    for order in permutations {
        let mut groups = Vec::new();
        for id in order { manage_images(&mut groups, Image { id, bits: bits[id as usize] }, &policy); }
        let mut partition: Vec<Vec<u8>> = groups.into_iter().map(|g| {
            let mut ids = vec![g.img.id]; ids.extend(g.similars.into_iter().map(|i| i.id));
            ids.sort(); ids
        }).collect();
        partition.sort();
        let expected = match order[0] {
            0 => vec![vec![0,1], vec![2]],
            1 => vec![vec![0,1,2]],
            _ => vec![vec![0], vec![1,2]],
        };
        assert_eq!(partition, expected);
        println!("{:?} -> {:?}", order, partition);
    }
}
'''
program = work / "group-order.rs"
program.write_text(prefix + body + suffix)
binary = (work / "group-order").resolve()
subprocess.run(["rustc", "--edition=2021", str(program), "-o", str(binary)], check=True)
result = subprocess.run([str(binary)], check=True, text=True, capture_output=True)
print(json.dumps({"controls": 6, "passed": 6, "partitions": result.stdout.splitlines(),
    "original_function_sha256": hashlib.sha256(body.encode()).hexdigest(),
    "original_source_sha256": hashlib.sha256(source).hexdigest(),
    "scope": "Original complete manage_images function body unchanged; synthetic four-bit Hamming image/settings/group adapters. Not original external img_hash, decoder, workers, Cargo package or GUI execution."}, indent=2))
