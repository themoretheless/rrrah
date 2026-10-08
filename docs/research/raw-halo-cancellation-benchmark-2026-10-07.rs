use std::time::Instant;
use std::hint::black_box;
use std::sync::atomic::{AtomicU64,Ordering};
#[derive(Debug)] enum GpuError { Cancelled }
fn baseline_tile(
    pixels: &[u16],
    width: u32,
    height: u32,
    tile_x: u32,
    tile_y: u32,
    tile_size: u32,
    halo: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u16>, GpuError> {
    let extent_u32 = tile_size.checked_add(halo.saturating_mul(2)).unwrap_or(0);
    let extent = usize::try_from(extent_u32).unwrap_or(0);
    let width = usize::try_from(width).unwrap_or(0);
    let height = usize::try_from(height).unwrap_or(0);
    let tile_x = usize::try_from(tile_x.saturating_mul(tile_size)).unwrap_or(0);
    let tile_y = usize::try_from(tile_y.saturating_mul(tile_size)).unwrap_or(0);
    let halo = usize::try_from(halo).unwrap_or(0);
    let mut output = vec![0_u16; extent.saturating_mul(extent)];
    if width == 0 || height == 0 || pixels.len() < width.saturating_mul(height) {
        return Ok(output);
    }

    // Every output row samples one contiguous source-row interval, with only
    // the left and right sensor edges requiring replicated values. Copy that
    // interval in bulk instead of repeating saturating coordinate arithmetic
    // and a two-dimensional source lookup for every sample. This preserves
    // the exact clamp-to-edge policy, including oversized edge tiles.
    let left_fill = halo.saturating_sub(tile_x).min(extent);
    let source_x = tile_x.saturating_sub(halo).min(width.saturating_sub(1));
    let copy_len = width
        .saturating_sub(source_x)
        .min(extent.saturating_sub(left_fill));
    for local_y in 0..extent {
        let source_y = tile_y
            .saturating_add(local_y)
            .saturating_sub(halo)
            .min(height.saturating_sub(1));
        let source_row = &pixels[source_y * width..(source_y + 1) * width];
        let output_row = &mut output[local_y * extent..(local_y + 1) * extent];

        if left_fill != 0 {
            output_row[..left_fill].fill(source_row[source_x]);
        }
        if copy_len != 0 {
            output_row[left_fill..left_fill + copy_len]
                .copy_from_slice(&source_row[source_x..source_x + copy_len]);
        }
        if left_fill + copy_len < extent {
            output_row[left_fill + copy_len..].fill(source_row[width - 1]);
        }
    }
    Ok(output)
}
fn tile_with_halo_with_cancel(
    pixels: &[u16],
    width: u32,
    height: u32,
    tile_x: u32,
    tile_y: u32,
    tile_size: u32,
    halo: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u16>, GpuError> {
    if cancelled() { return Err(GpuError::Cancelled); }
    let extent_u32 = tile_size.checked_add(halo.saturating_mul(2)).unwrap_or(0);
    let extent = usize::try_from(extent_u32).unwrap_or(0);
    let width = usize::try_from(width).unwrap_or(0);
    let height = usize::try_from(height).unwrap_or(0);
    let tile_x = usize::try_from(tile_x.saturating_mul(tile_size)).unwrap_or(0);
    let tile_y = usize::try_from(tile_y.saturating_mul(tile_size)).unwrap_or(0);
    let halo = usize::try_from(halo).unwrap_or(0);
    let mut output = vec![0_u16; extent.saturating_mul(extent)];
    if width == 0 || height == 0 || pixels.len() < width.saturating_mul(height) {
        return Ok(output);
    }

    // Every output row samples one contiguous source-row interval, with only
    // the left and right sensor edges requiring replicated values. Copy that
    // interval in bulk instead of repeating saturating coordinate arithmetic
    // and a two-dimensional source lookup for every sample. This preserves
    // the exact clamp-to-edge policy, including oversized edge tiles.
    let left_fill = halo.saturating_sub(tile_x).min(extent);
    let source_x = tile_x.saturating_sub(halo).min(width.saturating_sub(1));
    let copy_len = width
        .saturating_sub(source_x)
        .min(extent.saturating_sub(left_fill));
    for local_y in 0..extent {
        if local_y % 64 == 0 && cancelled() { return Err(GpuError::Cancelled); }
        let source_y = tile_y
            .saturating_add(local_y)
            .saturating_sub(halo)
            .min(height.saturating_sub(1));
        let source_row = &pixels[source_y * width..(source_y + 1) * width];
        let output_row = &mut output[local_y * extent..(local_y + 1) * extent];

        if left_fill != 0 {
            output_row[..left_fill].fill(source_row[source_x]);
        }
        if copy_len != 0 {
            output_row[left_fill..left_fill + copy_len]
                .copy_from_slice(&source_row[source_x..source_x + copy_len]);
        }
        if left_fill + copy_len < extent {
            output_row[left_fill + copy_len..].fill(source_row[width - 1]);
        }
    }
    if cancelled() { return Err(GpuError::Cancelled); }
    Ok(output)
}
fn main() {
 let generation=AtomicU64::new(1);
 let pixels:Vec<u16>=(0..2050*2050).map(|v|(v%65536) as u16).collect();
 for size in [512,2048] {
  let cancel=||generation.load(Ordering::Acquire)!=1;
  let expected=baseline_tile(&pixels,2050,2050,0,0,size,1,&cancel).unwrap();
  assert_eq!(tile_with_halo_with_cancel(&pixels,2050,2050,0,0,size,1,&cancel).unwrap(),expected);
  let mut old=Vec::new();let mut new=Vec::new();
  for iteration in 0..100 {
   for mode in [iteration%2,1-iteration%2] {
    let start=Instant::now();
    let result=if mode==0 {baseline_tile(black_box(&pixels),2050,2050,0,0,size,1,&cancel)} else {tile_with_halo_with_cancel(black_box(&pixels),2050,2050,0,0,size,1,&cancel)}.unwrap();
    black_box(&result);
    let elapsed=start.elapsed().as_secs_f64()*1000.;
    if iteration>=10 { if mode==0 {old.push(elapsed)} else {new.push(elapsed)} }
   }
  }
  old.sort_by(f64::total_cmp);new.sort_by(f64::total_cmp);
  println!("tile={} baseline_median_ms={:.6} cancellable_median_ms={:.6} baseline_p95_ms={:.6} cancellable_p95_ms={:.6} samples={}",size,old[45],new[45],old[85],new[85],old.len());
 }
}