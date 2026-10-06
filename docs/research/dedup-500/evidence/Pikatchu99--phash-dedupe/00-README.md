# phash-dedupe

Find near-duplicate images in a folder — CLI and TypeScript library, zero cloud
dependency, MIT licensed.

Built out of a very concrete need: sourcing sticker packs (memes, reaction
faces) from public search results for [SnapLover](https://github.com/Pikatchu99/snaplover)
turns up the same photo over and over, reposted under different filenames, in
different packs, sometimes recompressed or re-cropped by a few pixels. Byte-for-byte
duplicate detection (file hashing) misses all of those. `phash-dedupe` doesn't.

## How it works

The tool computes a **difference hash (dHash)** for every image:

1. Resize the image to a tiny `9x8` grayscale thumbnail (ignoring aspect ratio).
2. For every row, compare each pixel to the one immediately to its right: `1` if
   it's brighter, `0` otherwise. That's 8 comparisons × 8 rows = a 64-bit
   fingerprint.
3. Two images are "the same" if their fingerprints differ in only a handful of
   bits — measured as **Hamming distance** (how many bits differ).

dHash is a gradient-based hash: it survives re-compression, mild recropping,
and brightness/contrast changes far better than a naive average-hash (aHash),
because it encodes *relative* brightness between neighbouring pixels rather
than absolute brightness against a global average. It's not immune to false
positives, though — see [Limitations](#limitations) below before you wire
`--delete` into an unattended script.

## Install

Not published to npm yet — clone and build locally:

```bash
git clone git@github.com:Pikatchu99/phash-dedupe.git
cd phash-dedupe
pnpm install
pnpm build
```

Run the CLI with `node ./dist/cli.js <dir>`, or link it globally:

```bash
pnpm link --global
phash-dedupe <dir>
```

## CLI usage

```bash
phash-dedupe ./stickers
```

```
3 duplicate cluster(s), 4 file(s) redundant:

  keep   stickers/cats/angry-cat-05.jpg
  remove stickers/cats/angry-cat-19.jpg

  keep   stickers/cute/side-eye-01.jpg
  remove stickers/drama/dramatic-10.jpg
  remove stickers/drama/dramatic-12.jpg
  ...
```

Options:

| Flag                | Default | Meaning                                                                 |
| -------------------- | ------- | ------------------------------------------------------------------------ |
| `--threshold <n>`    | `6`     | Max Hamming distance (out of 64) to count as a match. Lower = stricter.   |
| `--hash-size <n>`    | `8`     | Hash resolution (`n x n` bits). Higher = more detail, less tolerant of noise. |
| `--no-recursive`     | off     | Don't scan subdirectories.                                                |
| `--json`             | off     | Print `{ clusters, toDelete }` as JSON instead of a text report.          |
| `--delete`           | off     | Delete every file in each cluster except the first alphabetically.       |
| `--dry-run`          | off     | Combine with `--delete` to preview what would be removed.                |

Recommended workflow: run without `--delete` first, read the report, adjust
`--threshold` if needed, then re-run with `--delete --dry-run` to confirm the
exact file list, and only then drop `--dry-run` for real.

### Choosing a threshold

| Threshold | What you get |
| --- | --- |
| 0–3  | Very safe. Almost never a false positive — same photo, different crop/compression. |
| 4–8  | Catches more reposts, but starts to flag unrelated images that just happen to share a similar brightness gradient (e.g. two different photos with a plain, brightly-lit background). Eyeball every match before trusting `--delete` at this range. |
| 9+   | Too loose for most real photo sets — mostly noise. |

## Library usage

```ts
import { findDuplicates } from "phash-dedupe";

const { clusters } = await findDuplicates("./stickers", { threshold: 6 });
for (const cluster of clusters) {
  console.log("near-duplicates:", cluster);
}
```

Lower-level building blocks are exported too, for incremental hashing, caching
hashes across runs, or comparing two separate folders against each other:

```ts
import { scanImages, hashImage, hammingDistance, findDuplicatePairs, clusterDuplicates } from "phash-dedupe";

const paths = await scanImages("./stickers");
const hashes = new Map(await Promise.all(paths.map(async (p) => [p, await hashImage(p)] as const)));
const pairs = findDuplicatePairs(hashes, 6);
const clusters = clusterDuplicates(pairs);
```

## Limitations

- **Flat/simple images degrade to false positives.** A dHash is built entirely
  from left-right brightness gradients; a solid-colour image (or a photo with
  a large flat background, like an unremoved green-screen) has no gradient
  anywhere and hashes toward `0`, which can spuriously match other
  low-detail images. Always eyeball threshold ≥7 matches before deleting.
- **O(n²) comparison.** Every hash is compared against every other hash. Fine
  for a few thousand images (a full sticker-pack sourcing run); for a much
  larger library you'd want to bucket hashes first (e.g. by a locality-sensitive
  prefix) before the pairwise pass — not implemented here.
- **Rotation and heavy crops aren't handled.** dHash is not rotation-invariant.
  A 90°-rotated repost of the same photo will not match.

## Development

```bash
pnpm install
pnpm test    # vitest
pnpm lint    # eslint
pnpm build   # tsc -> dist/
```

## License

MIT © Yemalin Modeste
