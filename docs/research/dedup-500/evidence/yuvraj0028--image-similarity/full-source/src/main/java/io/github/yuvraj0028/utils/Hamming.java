package io.github.yuvraj0028.utils;

/**
 * Utility class for calculating the Hamming Distance between two long integers.
 */
public class Hamming {
    // Private constructor to prevent external instantiation.
    Hamming(){}
    
    /**
     * Calculates the Hamming Distance (number of differing bits) between two 64-bit hashes.
     * It uses XOR (a ^ b) to find differing bits, and Long.bitCount() to count them.
     * * @param a The first hash value.
     * @param b The second hash value.
     * @return The number of differing bits.
     */
    public static int distanceLong(long a, long b){
        long x = a ^ b;
        // Long.bitCount(x) is the efficient way to count the set bits (1s) in x.
        return Long.bitCount(x);
    }
}