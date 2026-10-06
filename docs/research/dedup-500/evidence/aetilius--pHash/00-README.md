# pHash — the open source perceptual hash library

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)

pHash is a C++ library for computing **perceptual hashes** of images, video,
audio, and text. Unlike cryptographic hashes (which change completely with a
single bit flip), perceptual hashes change *gradually* as the input changes —
two files that look or sound the same to a human produce hash values that are
close to each other in Hamming distance, even after lossy re-encoding,
resizing, mild noise, or other content-preserving transformations.

This makes pHash suitable for:

- Near-duplicate image / video / audio detection
- Copy-detection and content-ID systems
- Reverse-image search backends
- Indexing media libraries against perturbation (re-encodes, resizes, brightness changes…)
- Plagiarism detection for text

---

## Hash families

| Function | What it hashes | Output | Robust to |
|---|---|---|---|
| `ph_dct_imagehash`     | Image (DCT)                       | 64-bit `ulong64`     | Resize, compression, mild brightness/contrast |
| `ph_image_digest` / `ph_compare_images` | Image (Radon transform + DCT, "Radish")  | 40-byte digest + cross-correlation | Rotation, resize |
| `ph_mh_imagehash`      | Image (Marr-Hildreth wavelet)     | 72-byte (576-bit)    | Resize, slight crop, JPEG noise |
| `ph_dct_videohash`     | Video (keyframe DCT hashes + LCS) | array of 64-bit hashes | Re-encoding, frame-rate change |
| `ph_audiohash`         | Audio (Bark-scale spectral, Haitsma-Kalker style) | array of 32-bit frames | Re-encoding, mild EQ |
| `ph_texthash`          | Text (k-gram winnowing)           | array of `(hash, offset)` | Insertion, deletion, edits |

Distances are computed with companion functions: `ph_hamming_distance`,
`ph_hammingdistance2`, `ph_crosscorr`, `ph_dct_videohash_dist`, and
`ph_audio_distance_ber` for the appropriate hash family. See
[`src/pHash.h`](src/pHash.h.cmake) and [`src/audiophash.h`](src/audiophash.h)
for the full API.

---

## Quick start

```sh
git clone https://github.com/aetilius/pHash.git
cd pHash
mkdir build && cd build
cmake -DPHASH_EXAMPLES=ON -DWITH_AUDIO_HASH=ON -DWITH_VIDEO_HASH=ON ..
make -j
sudo make install
```

This produces `libpHash.so` (or `.dylib` on macOS) plus a set of
demonstration binaries in `build/Release/`:

```
TestDCT          # ph_dct_imagehash on two directories of images
TestRadish       # ph_image_digest (radon-based) on one image
TestMH           # ph_mh_imagehash distance between two images
TestAudio        # ph_audiohash + ph_audio_distance_ber on two audio files
TestVideoHash    # ph_dct_videohash on two video files
```

---

## Hashing your first image

```cpp
#include <cstdio>
#include "pHash.h"

int main(int argc, char **argv) {
    if (argc < 3) return 1;

    ulong64 hashA = 0, hashB = 0;
    if (ph_dct_imagehash(argv[1], hashA) < 0) return 1;
    if (ph_dct_imagehash(argv[2], hashB) < 0) return 1;

    int distance = ph_hamming_distance(hashA, hashB);
    std::printf("%016llx vs %016llx -> Hamming distance %d / 64\n",
                (unsigned long long)hashA,
                (unsigned long long)hashB,
                distance);
    return 0;
}
```

Build with:

```sh
c++ -std=c++14 -O2 example.cpp -lpHash -lpng -ljpeg -ltiff -lz -o example
```

Rule of thumb for the DCT image hash: a Hamming distance of `0` means
the images are essentially identical; ≤ 10 means very similar; ≥ 20
typically means unrelated. Tune the threshold against your own data.

---

## Comparing audio

```cpp
#include <cstdio>
#include "audiophash.h"

int main(int argc, char **argv) {
    if (argc < 3) return 1;

    int sr = 8000, channels = 1;
    int Na = 0, Nb = 0;
    float *a = ph_readaudio(argv[1], sr, channels, NULL, Na);
    float *b = ph_readaudio(argv[2], sr, channels, NULL, Nb);
    if (!a || !b) return 1;

    int frames_a = 0, frames_b = 0;
    uint32_t *ha = ph_audiohash(a, Na, sr, frames_a);
    uint32_t *hb = ph_audiohash(b, Nb, sr, frames_b);

    int Nc = 0;
    double *C = ph_audio_distance_ber(ha, frames_a, hb, frames_b,
                                       /*threshold=*/0.30f,
                                       /*block_size=*/256, Nc);

    double best = 0;
    for (int i = 0; i < Nc; i++) if (C[i] > best) best = C[i];
    std::printf("confidence: %.3f (1.0 = identical, 0.5 = unrelated)\n", best);

    free(a); free(b); free(ha); free(hb); free(C);
    return 0;
}
```

---

## Dependencies

| Component      | Required when            | Debian/Ubuntu                                                                                  | macOS (Homebrew) |
|---|---|---|---|
| **Image hash** | Always (HAVE_IMAGE_HASH=ON by default) | `libpng-dev libjpeg-dev libtiff-dev`                                                           | `libpng libtiff libjpeg` |
| **Video hash** | `-DWITH_VIDEO_HASH=ON`   | `libavcodec-dev libavformat-dev libavutil-dev libswscale-dev`                                  | `ffmpeg` |
| **Audio hash** | `-DWITH_AUDIO_HASH=ON`   | `libsndfile1-dev libsamplerate0-dev libmpg123-dev`                                             | `libsndfile libsamplerate mpg123` |
| **HEIF/HEIC/AVIF** | `-DWITH_HEIF=ON`     | `libheif-dev`                                                                                  | `libheif` |
| **Threads**    | Always                   | pthreads from the standard libc                                                                | — (built-in) |

Note: pHash bundles [CImg](http://cimg.eu/) in `third-party/CImg/` — no separate install needed.

FFmpeg **5, 6, 7, and 8** are all supported.

---

## Build options

| CMake option            | Default | Purpose |
|---|---|---|
| `PHASH_DYNAMIC`         | `ON`    | Build the shared library |
| `PHASH_STATIC`          | `OFF`   | Build the static library |
| `WITH_AUDIO_HASH`       | `OFF`   | Compile `audiophash.cpp` and the audio hash API |
| `WITH_VIDEO_HASH`       | `OFF`   | Compile `cimgffmpeg.cpp` and the video hash API |
| `WITH_HEIF`             | `OFF`   | Decode `.heic` / `.heif` / `.avif` via libheif (issue #18) |
| `PHASH_EXAMPLES`        | `OFF`   | Build the `TestDCT` / `TestRadish` / etc. demos |
| `PHASH_BINDINGS`        | `OFF`   | Build language bindings (Java/C#/PHP) |

Example: image hash only, no examples:

```sh
cmake ..
```

Everything on:

```sh
cmake -DWITH_AUDIO_HASH=ON -DWITH_VIDEO_HASH=ON \
      -DPHASH_EXAMPLES=ON -DPHASH_BINDINGS=ON ..
```

---

## Language bindings

The `bindings/` directory ships wrappers for:

- **Java** (`bindings/java/`) — JNI bindings, see `bindings/java/README` for the JDK requirements.
- **C#** (`bindings/c_sharp/`) — P/Invoke wrapper for the C ABI.
- **PHP** (`bindings/php/`) — built against the PHP extension API.

Enable with `-DPHASH_BINDINGS=ON`.

---

## How pHash works (in one paragraph each)

- **DCT image hash** — Resize to 32×32 grayscale, take the 2D DCT,
  pick the 8×8 low-frequency block, threshold against the median of
  the AC coefficients. The 64-bit hash is robust to scaling, mild
  blurring, and small color/brightness changes.
- **Radial / Radish hash** — Compute Radon-style line integrals at
  N angles through the image, take the per-angle variance as a feature
  vector, DCT-compress it to 40 coefficients. Compare via circular
  cross-correlation, which gives rotation invariance.
- **Marr-Hildreth wavelet hash** — Convolve a 512×512 equalised
  grayscale image with a Marr (Mexican Hat) wavelet, tile the response
  into a 31×31 grid of block sums, and produce 576 bits by comparing
  each cell to its local 3×3 average.
- **DCT video hash** — Detect keyframes via a histogram-difference
  shot detector, hash each keyframe with the DCT image hash, and
  compare two videos via longest-common-subsequence over the hash
  arrays with a Hamming-distance threshold.
- **Audio hash** — Resample to 8 kHz mono, divide into overlapping
  ~0.37 s frames, compute Bark-scale spectral energy in 33 bands from
  300–2000 Hz, and emit one bit per adjacent band pair based on
  whether the per-band energy difference is increasing or decreasing
  between frames (Haitsma-Kalker fingerprint).
- **Text hash** — Compute a rolling 50-character k-gram hash, then
  apply Schleimer-Wilkerson-Aiken winnowing with a 100-k-gram window
  to select representative fingerprints.

For the academic context behind each method, see the papers cited in
[`src/pHash.h`](src/pHash.h.cmake) header comments and in the
[ChangeLog](ChangeLog).

---

## Project layout

```
src/                    Library source
  pHash.cpp             Image, video, and text hash code
  cimgffmpeg.{cpp,h}    FFmpeg wrapper for video keyframe extraction
  audiophash.{cpp,h}    Audio hash code
  ph_fft.{cpp,h}        Custom radix-2 FFT used by the audio hash
  pHash.h.cmake         Public header template (configured by CMake)
examples/               Demonstration programs (test_*.cpp)
bindings/               Java / C# / PHP wrappers
tests/                  (stub) unit tests
third-party/CImg/       Bundled CImg.h (image I/O + processing)
phash-win32/            MSVC project files (Windows)
```

---

## Authors

- **Evan Klinger** — `<eklinger@phash.org>`
- **D Grant Starkweather** — `<dstarkweather@phash.org>`

Project home: <http://www.phash.org/>

---

## License

GNU General Public License v3 or later. See [`COPYING`](COPYING) for
the full text.
