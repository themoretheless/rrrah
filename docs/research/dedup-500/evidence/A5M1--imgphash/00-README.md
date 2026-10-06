# imagehash_ffi
A high-performance Rust library providing C FFI bindings for perceptual image hashing algorithms. Optimized with safe memory boundaries and efficient matrix operations.
## Features
Supports multiple image hashing algorithms for deduplication and similarity search:
 * aHash (Average Hash)
 * dHash (Difference Hash)
 * pHash (Perceptual Hash via DCT)
 * wHash (Wavelet Hash using Haar transforms)
 * ColorHash (Color distribution hashing)

Build
`cargo build --release`

Output:
 * Linux / Android: `target/release/libimagehash_ffi.a / libimagehash_ffi.so`
 * Windows: `target/release/imagehash_ffi.lib / imagehash_ffi.dll`
 * macOS: `target/release/libimagehash_ffi.a / libimagehash_ffi.dylib`

## C API Reference

```c
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Hashing Algorithms
char* imagehash_average_hash(const uint8_t* data, uint32_t width, uint32_t height, uint32_t hash_size);
char* imagehash_dhash(const uint8_t* data, uint32_t width, uint32_t height, uint32_t hash_size);
char* imagehash_phash(const uint8_t* data, uint32_t width, uint32_t height, uint32_t hash_size, uint32_t highfreq_factor);
char* imagehash_whash_haar(const uint8_t* data, uint32_t width, uint32_t height, uint32_t hash_size, int32_t image_scale);
char* imagehash_colorhash(const uint8_t* data, uint32_t width, uint32_t height, uint32_t binbits);

// Utility & Memory Management
uint32_t imagehash_hamming_distance(const char* hex1, const char* hex2);
void imagehash_free_string(char* s);

#ifdef __cplusplus
}
#endif
```

## Usage & Memory Management
The library expects raw RGB24 pixel data (3 bytes per pixel: Red, Green, Blue).
Important: Any string pointer (char*) returned by the hashing functions is allocated by Rust. You must pass it to imagehash_free_string when you are done to prevent memory leaks. Do not use standard C free().

```c
#include <stdio.h>
#include <stdint.h>

// Example: Using pHash to compare two images
void compare_images(const uint8_t* rgb1, const uint8_t* rgb2, uint32_t w, uint32_t h) {
    // Generate perceptual hashes
    char* hash1 = imagehash_phash(rgb1, w, h, 8, 4);
    char* hash2 = imagehash_phash(rgb2, w, h, 8, 4);
    
    // Calculate Hamming distance
    uint32_t distance = imagehash_hamming_distance(hash1, hash2);
    printf("Hash 1: %s\nHash 2: %s\nDistance: %u\n", hash1, hash2, distance);
    
    // Safely free Rust-allocated memory
    imagehash_free_string(hash1);
    imagehash_free_string(hash2);
}

```

### AI DISCLOSURE
[![model](https://img.shields.io/badge/model-red?style=flat&logo=github&logoColor=red&labelColor=black)](https://huggingface.co/OBLITERATUS/gemma-4-E4B-it-OBLITERATED)
the following model generated this readme, running on a gpu server running entirely on solar. 
