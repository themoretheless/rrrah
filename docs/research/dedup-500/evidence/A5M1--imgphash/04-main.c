#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include "capi_phash2.h" 

#define WIDTH 256
#define HEIGHT 256

void generate_pattern_a(uint8_t* data, uint32_t width, uint32_t height) {
    for (uint32_t y = 0; y < height; ++y) {
        for (uint32_t x = 0; x < width; ++x) {
            uint32_t idx = (y * width + x) * 3;
            data[idx]     = (uint8_t)(x % 256);
            data[idx + 1] = (uint8_t)(y % 256);
            data[idx + 2] = (uint8_t)((x + y) % 256);
        }
    }
}

void generate_pattern_b(uint8_t* data, uint32_t width, uint32_t height) {
    for (uint32_t y = 0; y < height; ++y) {
        for (uint32_t x = 0; x < width; ++x) {
            uint32_t idx = (y * width + x) * 3;
            data[idx]     = (uint8_t)(x % 256);
            data[idx + 1] = (uint8_t)(y % 256);
            data[idx + 2] = (uint8_t)(255 - ((x + y) % 256));
        }
    }
}

int main() {
    size_t img_size = WIDTH * HEIGHT * 3;
    uint8_t* img_a = (uint8_t*)malloc(img_size);
    uint8_t* img_b = (uint8_t*)malloc(img_size);

    if (!img_a || !img_b) {
        return 1;
    }

    generate_pattern_a(img_a, WIDTH, HEIGHT);
    generate_pattern_b(img_b, WIDTH, HEIGHT);

    printf("--- ImageHash FFI Test ---\n\n");

    char* ahash_a = imagehash_average_hash(img_a, WIDTH, HEIGHT, 8);
    char* ahash_b = imagehash_average_hash(img_b, WIDTH, HEIGHT, 8);
    printf("[aHash] Image A: %s\n", ahash_a);
    printf("[aHash] Image B: %s\n", ahash_b);
    printf("[aHash] Distance: %u\n\n", imagehash_hamming_distance(ahash_a, ahash_b));

    char* phash_a = imagehash_phash(img_a, WIDTH, HEIGHT, 8, 4);
    char* phash_b = imagehash_phash(img_b, WIDTH, HEIGHT, 8, 4);
    printf("[pHash] Image A: %s\n", phash_a);
    printf("[pHash] Image B: %s\n", phash_b);
    printf("[pHash] Distance: %u\n\n", imagehash_hamming_distance(phash_a, phash_b));

    char* dhash_a = imagehash_dhash(img_a, WIDTH, HEIGHT, 8);
    char* dhash_b = imagehash_dhash(img_b, WIDTH, HEIGHT, 8);
    printf("[dHash] Image A: %s\n", dhash_a);
    printf("[dHash] Image B: %s\n", dhash_b);
    printf("[dHash] Distance: %u\n\n", imagehash_hamming_distance(dhash_a, dhash_b));

    imagehash_free_string(ahash_a);
    imagehash_free_string(ahash_b);
    imagehash_free_string(phash_a);
    imagehash_free_string(phash_b);
    imagehash_free_string(dhash_a);
    imagehash_free_string(dhash_b);

    free(img_a);
    free(img_b);

    return 0;
}
