import { hashImage, type HashOptions, type PerceptualHash } from "./hash.js";
import { scanImages, type ScanOptions } from "./scan.js";
import { clusterDuplicates, findDuplicatePairs, type DuplicatePair } from "./cluster.js";

export interface FindDuplicatesOptions extends HashOptions, ScanOptions {
  /**
   * Maximum Hamming distance (out of `hashSize * hashSize` bits, 64 by
   * default) for two images to be considered near-duplicates. Lower is
   * stricter. 0 means "only exact-content matches" (after the hash's
   * resize+grayscale step, so still not byte-identical). In practice:
   * - 0-3: very safe, almost never a false positive.
   * - 4-8: catches recompressed/recropped reposts, but starts to include
   *   occasional false positives on visually simple images (flat colour
   *   backgrounds, high-contrast silhouettes) — verify by eye before deleting.
   * - 9+: too loose for most photo sets, mostly noise.
   * Defaults to 6.
   */
  threshold?: number;
  /** Called after each image is hashed, useful for a progress indicator. */
  onProgress?: (done: number, total: number, path: string) => void;
}

export interface FindDuplicatesResult {
  /** Every scanned file mapped to its perceptual hash. */
  hashes: Map<string, PerceptualHash>;
  /** All pairs within the threshold, closest first. */
  pairs: DuplicatePair[];
  /** Pairs grouped transitively into clusters of 2+ near-duplicate files. */
  clusters: string[][];
}

/**
 * End-to-end convenience wrapper: scans `dir` for images, hashes each one,
 * and returns both the raw near-duplicate pairs and them grouped into
 * clusters. This is almost always what you want; use {@link scanImages},
 * {@link hashImage}, {@link findDuplicatePairs} and {@link clusterDuplicates}
 * directly if you need to hash incrementally, cache hashes across runs, or
 * compare two separate folders against each other.
 */
export async function findDuplicates(dir: string, options: FindDuplicatesOptions = {}): Promise<FindDuplicatesResult> {
  const threshold = options.threshold ?? 6;
  const paths = await scanImages(dir, options);

  const hashes = new Map<string, PerceptualHash>();
  for (let i = 0; i < paths.length; i++) {
    const path = paths[i]!;
    hashes.set(path, await hashImage(path, options));
    options.onProgress?.(i + 1, paths.length, path);
  }

  const pairs = findDuplicatePairs(hashes, threshold);
  const clusters = clusterDuplicates(pairs);

  return { hashes, pairs, clusters };
}
