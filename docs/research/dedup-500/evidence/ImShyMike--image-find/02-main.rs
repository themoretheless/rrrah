use image::DynamicImage;
use img_hash::{HashAlg, HasherConfig, ImageHash};
use rayon::prelude::*;
use std::io::Write;
use std::sync::Arc;
use std::{
    ffi::OsStr,
    fs::{File, read_dir},
    path::PathBuf,
    thread,
};

const MAX_SIZE: u32 = 256;
const BATCH_SIZE: usize = 64;

#[inline(always)]
fn is_supported_image_ext(ext: &OsStr) -> bool {
    let ext_bytes = ext.as_encoded_bytes();
    matches!(
        ext_bytes,
        b"png"
            | b"PNG"
            | b"jpg"
            | b"JPG"
            | b"jpeg"
            | b"JPEG"
            | b"bmp"
            | b"BMP"
            | b"webp"
            | b"WEBP"
            | b"tiff"
            | b"TIFF"
            | b"gif"
            | b"GIF"
    )
}

#[inline(always)]
fn get_image_from_path(path: &PathBuf) -> Result<DynamicImage, image::ImageError> {
    let reader = image::ImageReader::open(path)?;
    reader.with_guessed_format()?.decode()
}

#[inline(always)]
fn create_hasher() -> img_hash::Hasher {
    HasherConfig::new()
        .hash_alg(HashAlg::Gradient)
        .hash_size(8, 8)
        .to_hasher()
}

#[inline(always)]
fn compute_hash(img: DynamicImage) -> ImageHash {
    let hasher = create_hasher();

    // Resize only if necessary
    let processed_img = if img.width() > MAX_SIZE || img.height() > MAX_SIZE {
        img.resize(MAX_SIZE, MAX_SIZE, image::imageops::FilterType::Triangle)
    } else {
        img
    };

    let rgb = processed_img.to_rgb8();
    let buf =
        img_hash::image::ImageBuffer::from_raw(rgb.width(), rgb.height(), rgb.into_raw()).unwrap();
    let dyn_img = img_hash::image::DynamicImage::ImageRgb8(buf);

    hasher.hash_image(&dyn_img)
}

#[inline(always)]
fn calculate_similarity(hash1: &ImageHash, hash2: &ImageHash) -> f32 {
    let distance = hash1.dist(hash2);
    1.0 - (distance as f32 * 0.015625) // 1/64 = 0.015625
}

fn load_wanted_images(
    wanted_images_path: &str,
) -> Result<Vec<ImageHash>, Box<dyn std::error::Error>> {
    let wanted_paths: Vec<PathBuf> = read_dir(wanted_images_path)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(is_supported_image_ext))
        .collect();

    println!("Loading {} wanted images...", wanted_paths.len());

    let wanted_hashes: Vec<ImageHash> = wanted_paths
        .par_chunks(BATCH_SIZE)
        .flat_map(|chunk| {
            chunk
                .par_iter()
                .enumerate()
                .filter_map(|(i, path)| match get_image_from_path(path) {
                    Ok(img) => {
                        println!(
                            "Loaded {}: {}",
                            i,
                            path.file_name().unwrap().to_string_lossy()
                        );
                        Some(compute_hash(img))
                    }
                    Err(e) => {
                        eprintln!("Failed to parse file {:?}: {}", path, e);
                        None
                    }
                })
        })
        .collect();

    Ok(wanted_hashes)
}

#[inline(always)]
fn find_max_similarity(target_hash: &ImageHash, wanted_hashes: &[ImageHash]) -> f32 {
    wanted_hashes
        .iter()
        .map(|w_hash| calculate_similarity(target_hash, w_hash))
        .fold(0.0f32, f32::max)
}

fn process_search_directory(
    search_folder: &str,
    wanted_hashes: &[ImageHash],
    threshold: f32,
    found_matches_folder: Option<&str>,
) -> Result<Vec<(PathBuf, f32)>, Box<dyn std::error::Error>> {
    let search_path = PathBuf::from(search_folder);
    if !search_path.is_dir() {
        return Err("Search directory is not valid".into());
    }

    // Create destination folder if specified
    if let Some(folder) = found_matches_folder {
        std::fs::create_dir_all(folder)?;
    }

    // Pre-collect all valid image paths
    let image_paths: Vec<PathBuf> = read_dir(&search_path)?
        .filter_map(|entry_result| {
            let entry = entry_result.ok()?;
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(is_supported_image_ext) {
                Some(path)
            } else {
                None
            }
        })
        .collect();
 
    println!(
        "Processing {} images in search directory...",
        image_paths.len()
    );

    // Process in batches
    let found_results: Vec<(PathBuf, f32)> = image_paths
        .par_chunks(BATCH_SIZE)
        .flat_map(|chunk| {
            let wanted_hashes = Arc::new(wanted_hashes);
            chunk.par_iter().filter_map(move |path| {
                match get_image_from_path(path) {
                    Ok(img) => {
                        let hash = compute_hash(img);
                        let max_sim = find_max_similarity(&hash, &wanted_hashes);

                        // Handle match
                        if max_sim >= threshold {
                            println!(
                                "Match {:.2}: {}",
                                max_sim,
                                path.file_name().unwrap().to_string_lossy()
                            );

                            // Copy file if destination folder is specified
                            if let Some(folder) = found_matches_folder {
                                if let Some(file_name) = path.file_name() {
                                    let dest_path = PathBuf::from(folder).join(file_name);
                                    if let Err(e) = std::fs::copy(path, &dest_path) {
                                        eprintln!("Failed to copy file {:?}: {}", path, e);
                                    }
                                }
                            }

                            Some((path.clone(), max_sim))
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to parse file {:?}: {}", path, e);
                        None
                    }
                }
            })
        })
        .collect();

    Ok(found_results)
}

fn write_results(found_results: &[(PathBuf, f32)]) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir().unwrap();
    let parent_path = cwd.parent().unwrap();
    let csv_path = parent_path.join("found.csv");

    let mut output = std::io::BufWriter::new(File::create(&csv_path)?);
    writeln!(output, "id,file,path,similarity")?;

    // Pre-allocate string capacity
    let mut line_buffer = String::with_capacity(512);

    for (img_path, similarity) in found_results {
        line_buffer.clear();
        line_buffer.push_str(&img_path.file_stem().unwrap().to_string_lossy());
        line_buffer.push(',');
        line_buffer.push_str(&img_path.file_name().unwrap().to_string_lossy());
        line_buffer.push(',');
        line_buffer.push_str(&img_path.canonicalize()?.to_string_lossy());
        line_buffer.push(',');
        line_buffer.push_str(&format!("{:.4}", similarity));
        writeln!(output, "{}", line_buffer)?;
    }

    output.flush()?;

    println!(
        "Found {} images. Details written to {}",
        found_results.len(),
        csv_path.to_string_lossy()
    );

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use half of available threads
    let available = thread::available_parallelism().unwrap().get();
    let default_threads = (available / 2).max(1);
    let num_threads = std::env::var("RAYON_NUM_THREADS")
        .unwrap_or_else(|_| default_threads.to_string())
        .parse()
        .unwrap_or(default_threads);

    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .expect("Failed to build Rayon thread pool");

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        println!(
            "Usage: {} <wanted_images_folder> <search_folder> [<similarity_threshold>] [<found_matches_folder>]",
            args[0]
        );
        return Ok(());
    }

    let wanted_images = &args[1];
    let search_folder = &args[2];
    let threshold: f32 = if args.len() < 4 {
        0.9
    } else {
        args[3].parse().unwrap_or(90) as f32 / 100.0
    };
    let found_matches_folder = args.get(4).map(|s| s.as_str());

    println!("Using threshold: {:.2}", threshold);
    println!("Using {} threads", num_threads);

    let start_time = std::time::Instant::now();

    // Load and hash wanted images
    let wanted_hashes = load_wanted_images(wanted_images)?;
    println!(
        "Loaded {} wanted image hashes in {:.2}s",
        wanted_hashes.len(),
        start_time.elapsed().as_secs_f32()
    );

    // Process search directory
    let found_results = process_search_directory(
        search_folder,
        &wanted_hashes,
        threshold,
        found_matches_folder,
    )?;

    // Write results
    write_results(&found_results)?;

    println!(
        "Total execution time: {:.2}s",
        start_time.elapsed().as_secs_f32()
    );

    Ok(())
}
