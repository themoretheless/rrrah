import sharp from "sharp";

/**
 * A perceptual hash encoded as a bigint (bit `i` set means pixel `i` was
 * "brighter than its neighbour" — see {@link hashImage}).
 */
export type PerceptualHash = bigint;

export interface HashOptions {
  /**
   * Hash resolution. The hash has `hashSize * hashSize` bits — higher values
   * capture more detail (fewer false-positive matches) but tolerate less
   * compression/resizing noise between two copies of the same image.
   * Defaults to 8 (a 64-bit hash), which is the standard dHash size.
   */
  hashSize?: number;
}

/**
 * Computes a difference hash (dHash) for an image file.
 *
 * dHash resizes the image to `(hashSize + 1) x hashSize` in grayscale, then
 * for every row sets one bit per pixel: 1 if that pixel is brighter than the
 * pixel immediately to its right, 0 otherwise. This gradient-based approach
 * is robust to brightness/contrast changes and mild re-compression — the
 * kind of noise you get from the same photo being re-saved, re-cropped
 * slightly, or re-uploaded to a different site — which makes it a better
 * default than the simpler average-hash (aHash) for spotting reposted memes
 * and stock photos.
 *
 * @see https://www.hackerfactor.com/blog/index.php%3F/archives/529-Kind-of-Like-That.html
 */
export async function hashImage(filePath: string, options: HashOptions = {}): Promise<PerceptualHash> {
  const hashSize = options.hashSize ?? 8;
  const width = hashSize + 1;

  const { data } = await sharp(filePath)
    .grayscale()
    .resize(width, hashSize, { fit: "fill" })
    .raw()
    .toBuffer({ resolveWithObject: true });

  let hash = 0n;
  for (let row = 0; row < hashSize; row++) {
    for (let col = 0; col < hashSize; col++) {
      // Safe by construction: `data` is exactly `width * hashSize` bytes
      // (raw grayscale buffer from the resize above), and row/col stay
      // within those bounds.
      const left = data[row * width + col]!;
      const right = data[row * width + col + 1]!;
      hash <<= 1n;
      if (left > right) hash |= 1n;
    }
  }
  return hash;
}

/** Number of bits that differ between two hashes — 0 means identical hashes. */
export function hammingDistance(a: PerceptualHash, b: PerceptualHash): number {
  let diff = a ^ b;
  let distance = 0;
  while (diff > 0n) {
    distance += Number(diff & 1n);
    diff >>= 1n;
  }
  return distance;
}
