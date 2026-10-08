use rrrah_dedup::{animated::AnimationBudget, decode::decode_selected_frame_bounded, exact::ContentSnapshot};
use std::io::Write;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 2);
    let request = rrrah_decode::DecodeRequest::new(&args[0]);
    let snapshot = ContentSnapshot::read(&request.path, 5242880, || false).unwrap();
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let image = decode_selected_frame_bounded(
        &request,
        AnimationBudget {
            max_frames: 1,
            max_pixels: 6400000,
            max_file_bytes: 5242880,
        },
        &budget,
        || false,
    )
    .unwrap();
    let view = image.view(|| false).unwrap();
    let (w, h) = view.dimensions();
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&args[1])
        .unwrap();
    let mut file = std::io::BufWriter::new(file);
    file.write_all(b"RRRAH-RANK-RGBA32-V1\n").unwrap();
    file.write_all(&w.to_le_bytes()).unwrap();
    file.write_all(&h.to_le_bytes()).unwrap();
    for y in 0..h {
        for x in 0..w {
            for value in view.rgba(x, y).unwrap() {
                file.write_all(&value.to_le_bytes()).unwrap();
            }
        }
    }
    file.flush().unwrap();
    snapshot.verify(|| false).unwrap();
    drop(image);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"width\":{w},\"height\":{h},\"managed_used\":0,\"managed_peak\":{}}}",
        budget.peak()
    );
}
