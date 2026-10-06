#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @file imagehash.h
 * @brief C Interface for perceptual image hashing algorithms.
 */

// Hashing Algorithms

/**
 * @brief Computes the Average Hash (aHash) of an image.
 * 
 * @details Downscales the image, converts it to grayscale, computes the mean pixel value, 
 *          and generates a hash based on whether each pixel is above or below the mean.
 * 
 * @param data Pointer to the raw image pixel data (typically RGBA or RGB format).
 * @param width Width of the image in pixels.
 * @param height Height of the image in pixels.
 * @param hash_size The size of the hash to produce (resulting hash length depends on implementation).
 * @return char* A dynamically allocated null-terminated hexadecimal string representing the hash. 
 *         Must be freed using imagehash_free_string().
 */
char* imagehash_average_hash(const uint8_t* data, uint32_t width, uint32_t height, uint32_t hash_size);

/**
 * @brief Computes the Difference Hash (dHash) of an image.
 * 
 * @details Focuses on the gradient/changes between adjacent pixels by downscaling 
 *          to a small size and comparing each pixel to its horizontal neighbor.
 * 
 * @param data Pointer to the raw image pixel data.
 * @param width Width of the image in pixels.
 * @param height Height of the image in pixels.
 * @param hash_size The size of the hash to produce.
 * @return char* A dynamically allocated null-terminated hexadecimal string representing the hash. 
 *         Must be freed using imagehash_free_string().
 */
char* imagehash_dhash(const uint8_t* data, uint32_t width, uint32_t height, uint32_t hash_size);

/**
 * @brief Computes the Perceptual Hash (pHash) of an image using the Discrete Cosine Transform (DCT).
 * 
 * @details Reduces the image to a low-frequency spectrum using DCT to capture structural features 
 *          that are robust against minor modifications and compression.
 * 
 * @param data Pointer to the raw image pixel data.
 * @param width Width of the image in pixels.
 * @param height Height of the image in pixels.
 * @param hash_size The size of the hash to produce.
 * @param highfreq_factor Factor used to multiply the size for high-frequency coefficient sampling.
 * @return char* A dynamically allocated null-terminated hexadecimal string representing the hash. 
 *         Must be freed using imagehash_free_string().
 */
char* imagehash_phash(const uint8_t* data, uint32_t width, uint32_t height, uint32_t hash_size, uint32_t highfreq_factor);

/**
 * @brief Computes the Wavelet Hash (wHash) of an image using the Haar wavelet transform.
 * 
 * @details Decomposes the image into frequency sub-bands using the Haar wavelet 
 *          to emphasize structural approximations.
 * 
 * @param data Pointer to the raw image pixel data.
 * @param width Width of the image in pixels.
 * @param height Height of the image in pixels.
 * @param hash_size The size of the hash to produce.
 * @param image_scale Scale factor applied to the image during transformation.
 * @return char* A dynamically allocated null-terminated hexadecimal string representing the hash. 
 *         Must be freed using imagehash_free_string().
 */
char* imagehash_whash_haar(const uint8_t* data, uint32_t width, uint32_t height, uint32_t hash_size, int32_t image_scale);

/**
 * @brief Computes a Color-based Hash (colorhash) of an image.
 * 
 * @details Captures the color distribution/palette of the image rather than structural edges.
 * 
 * @param data Pointer to the raw image pixel data.
 * @param width Width of the image in pixels.
 * @param height Height of the image in pixels.
 * @param binbits Number of bits per color bin used in quantization.
 * @return char* A dynamically allocated null-terminated hexadecimal string representing the hash. 
 *         Must be freed using imagehash_free_string().
 */
char* imagehash_colorhash(const uint8_t* data, uint32_t width, uint32_t height, uint32_t binbits);

// Utility & Memory Management

/**
 * @brief Calculates the Hamming distance between two hexadecimal hash strings.
 * 
 * @details Measures the number of differing bits between two hashes, 
 *          which indicates how visually similar the corresponding images are.
 * 
 * @param hex1 Pointer to the first null-terminated hexadecimal hash string.
 * @param hex2 Pointer to the second null-terminated hexadecimal hash string.
 * @return uint32_t The computed Hamming distance (lower values indicate higher similarity).
 */
uint32_t imagehash_hamming_distance(const char* hex1, const char* hex2);

/**
 * @brief Frees a string allocated by the imagehash library.
 * 
 * @details Safely deallocates memory for strings returned by hash generation functions 
 *          to prevent memory leaks.
 * 
 * @param s Pointer to the string to be freed. Safe to pass NULL.
 */
void imagehash_free_string(char* s);

#ifdef __cplusplus
}
#endif
