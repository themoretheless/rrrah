<p align="center">
  <img src="assets/banner.png" alt="phasher" width="100%">
</p>

<p align="center">
  <b>Fast CLI tool for finding duplicates, clustering, classifying, and sorting images using perceptual hashing.</b>
</p>

<p align="center">
  <a href="#installation">Install</a> •
  <a href="#features">Features</a> •
  <a href="#usage">Usage</a> •
  <a href="#how-it-works">How it works</a> •
  <a href="#contributing">Contributing</a>
</p>

---

## What problems does this solve?

If you've worked with large image datasets, you've probably hit at least one of these:

**"I have 80K images and I know there are tons of duplicates, but I can't diff them by filename."**
Options [2] and [3] find perceptual duplicates regardless of filename, format, or resolution. Two JPEGs of the same page saved at different quality? Caught.

**"I need to split a dataset into categories but I only have ~200 labeled examples per class."**
Option [6] takes your small reference folders and classifies the entire unlabeled pool against them using visual similarity. No model training needed.

**"I built a classifier and the accuracy is suspiciously high — I think my train/test split has leaking duplicates."**
Option [7] compares multiple folders and flags every image that appears in more than one. One run tells you exactly where the contamination is.

**"I have thousands of images to review manually and I keep jumping between completely unrelated screenshots."**
Option [4] reorders your folder so visually similar images are next to each other. Sort by Date Modified and scroll through — similar images are grouped together.

**"My clustering pipeline works but the pairwise comparison takes forever on 50K+ images."**
Everything runs through vectorized NumPy XOR + byte-level popcount. 75K x 300 comparisons finish in ~2 seconds instead of minutes.

---

## Features

<p align="center">
  <img src="assets/features.png" alt="Feature overview" width="100%">
</p>

| # | Mode | Description |
|---|------|------------|
| 1 | **Search similar** | Find images in a folder that look like a query image |
| 2 | **Find duplicates** | Detect identical or near-identical copies |
| 3 | **Cluster images** | Group all images by visual similarity |
| 4 | **Similarity sort** | Reorder images so consecutive files look similar |
| 5 | **Match folder vs folder** | Find which target images match reference examples |
| 6 | **Multi-folder classify** | Sort a target folder into multiple named categories |
| 7 | **Cross-folder overlap** | Detect contamination / duplicates across folders |
| 8 | **Compute & cache hashes** | Pre-hash a folder for faster subsequent operations |
| 9 | **Cache stats** | Inspect and clean the hash cache |

---

## Installation

```bash
git clone https://github.com/AbdelmoussaouirEz/phasher.git
cd phasher
pip install -r requirements.txt
```

`rich` is optional — the tool falls back to plain terminal output without it.

## Usage

```bash
python run.py
```

```
+------------------------------------------------------------------+
|                          phasher                                 |
|                                                                  |
|  -- Single Folder -----------                                    |
|  [1] Search similar        find images similar to a query        |
|  [2] Find duplicates       find identical / near-identical       |
|  [3] Cluster images        group all by similarity               |
|  [4] Similarity sort       reorder by visual similarity          |
|                                                                  |
|  -- Multi Folder ------------                                    |
|  [5] Match folder vs folder                                      |
|  [6] Multi-folder classify                                       |
|  [7] Cross-folder overlap                                        |
|                                                                  |
|  -- Utilities ---------------                                    |
|  [8] Compute & cache hashes                                      |
|  [9] Cache stats                                                 |
|  [0] Exit                                                        |
+------------------------------------------------------------------+
```

---

## How it works

### pHash pipeline

<p align="center">
  <img src="assets/phash_pipeline.png" alt="pHash pipeline" width="100%">
</p>

Perceptual hashing converts an image into a compact bit string that captures its visual structure. Two images that *look* similar produce similar hashes, even if they differ in resolution, compression, or minor edits.

### Hamming distance

<p align="center">
  <img src="assets/hamming_distance.png" alt="Hamming distance explained" width="100%">
</p>

The Hamming distance between two hashes counts how many bits differ. The XOR of both hashes lights up exactly the differing bits, count them and you have the distance.

### Vectorized comparison engine

Instead of comparing hashes one pair at a time in Python, the tool packs all hashes into a 2D NumPy `uint64` matrix and runs:

```
XOR  →  view as uint8 bytes  →  lookup-table popcount  →  sum per row
```

This runs entirely in NumPy's C layer. The speedup over naive Python loops scales with dataset size:

<p align="center">
  <img src="assets/performance.png" alt="Performance comparison" width="100%">
</p>

### Union-Find clustering

Duplicate detection and clustering use a Union-Find (disjoint set) structure. After the vectorized pass identifies all pairs within the Hamming threshold, Union-Find merges them into connected components in near-linear time.

### Greedy nearest-neighbor sort

The similarity sort walks through the dataset greedily: starting from an arbitrary image, it always picks the closest unvisited image as the next step. The result is a path where consecutive images have minimal Hamming distance.

---

## Example workflows

### Deduplicate a scraped dataset

``` python
Choose [0-9]: 2

Folder to scan for duplicates: ./screenshots
Hamming threshold [0]: 2
Hash size [16]: 16

[INFO] Found 74832 images
[OK]   Found 1847 duplicate groups (5219 images, 3372 removable)
```

### Classify unlabeled images using reference folders

```python
Choose [0-9]: 6

How many reference categories? [3]: 4
Reference folder 1: ./refs/cars        Label: cars
Reference folder 2: ./refs/animals     Label: animals
Reference folder 3: ./refs/objects     Label: objects
Reference folder 4: ./refs/humans      Label: humans

Target folder: ./dataset/unlabeled

+----------------+-----------+---------+
| Category       | Reference | Matched |
+----------------+-----------+---------+
| cars           |       312 |    8441 |
| animals        |       287 |    6203 |
| objects        |       154 |    2891 |
| humans         |       203 |    4127 |
| UNMATCHED      |           |   53170 |
| TOTAL          |       956 |   74832 |
+----------------+-----------+---------+
```

### Sort a folder for visual review

``` Python
Choose [0-9]: 4

Folder to sort: ./dataset/cars

[INFO] Running nearest-neighbor ordering...
[INFO]   Avg consecutive distance: 3.2
[INFO]   Max consecutive distance: 47
[OK]   Sorted 12450 images -> ./dataset/cars_sorted/
[INFO] Sort by 'Date Modified' to see the similarity order.
```

### Check for train/test leakage

```python
Choose [0-9]: 7

Folder 1: ./split/train      Label: train
Folder 2: ./split/val        Label: val
Folder 3: ./split/test       Label: test

+----------+----------+-------+-------+---------------+
| Folder A | Folder B | Pairs | Exact | Status        |
+----------+----------+-------+-------+---------------+
| train    | val      |    42 |    18 | CONTAMINATED  |
| train    | test     |     0 |     0 | Clean         |
| val      | test     |     3 |     1 | CONTAMINATED  |
+----------+----------+-------+-------+---------------+
```

---

## Pipeline overview

<p align="center">
  <img src="assets/workflow.png" alt="Dataset cleaning pipeline" width="100%">
</p>

---

## Configuration

| Parameter | Default | Notes |
|-----------|---------|-------|
| `hash_size` | 16 | 16x16 = 256-bit hash. Use 8 for speed, 16 for accuracy |
| `hamming_threshold` | 6 | Max Hamming distance to consider "similar". Lower = stricter |
| `workers` | 8 | Parallel threads for hashing |

### Hash cache

Computed hashes are saved as `.phash_cache.json` in each scanned folder. Subsequent runs skip already-hashed images. The cache auto-invalidates when you change `hash_size`.

---

## Performance

Tested on ~75,000 thumbnail images (mixed PNG/JPG, ~200x150 avg):

| Operation | Time | Notes |
|-----------|------|-------|
| Hash 75K images (cold) | ~90s | 8 threads, hash_size=16 |
| Hash 75K images (cached) | <1s | JSON cache hit |
| 300 refs x 75K targets | ~2s | Vectorized matching |
| Pairwise 75K x 75K | ~45s | Full clustering |
| Nearest-neighbor sort 12K | ~8s | Greedy walk |

---

## Project structure

```
phasher/
├── __init__.py
├── __main__.py          # entry point (python -m phasher)
├── config.py            # constants and defaults
├── ui.py                # terminal I/O (rich / plain fallback)
├── hashing.py           # pHash computation + cache
├── hamming.py           # vectorized Hamming distance engine
├── union_find.py        # Union-Find data structure
├── commands/
│   ├── search.py        # [1] search similar
│   ├── duplicates.py    # [2] find duplicates
│   ├── cluster.py       # [3] cluster images
│   ├── sort.py          # [4] similarity sort
│   ├── match.py         # [5] folder vs folder
│   ├── classify.py      # [6] multi-folder classify
│   ├── overlap.py       # [7] cross-folder overlap
│   └── cache.py         # [8] compute hashes / [9] cache stats
├── requirements.txt
├── assets/              # README diagrams
├── .gitignore
├── LICENSE
└── README.md
```

---

## Contributing

Found a bug? Have a feature idea? Contributions are welcome.

- **Bug reports** — Open an issue with steps to reproduce and your Python/OS version.
- **Feature requests** — Open an issue describing what you'd like and why. Happy to discuss before you start coding.
- **Pull requests** — Fork the repo, make your changes, and open a PR. Keep it focused — one feature or fix per PR.
- **Questions** — If something is unclear or you're not sure how to use a feature, open an issue. No question is too small.

If this tool saved you some time, a star would be appreciated — it helps others find it.

---

## License

MIT
