package io.github.yuvraj0028.models;

/**
 * Defines the supported types of perceptual hashes for image similarity calculation.
 */
public enum HashType {
    
    /** Perceptual Hash (based on DCT). */
    PHASH,
    
    /** Difference Hash (based on pixel gradients). */
    DHASH,
    
    /** Block Mean Hash. */
    BLOCKHASH
}
