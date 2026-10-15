package io.github.yuvraj0028.service;

import java.io.File;
import java.io.IOException;
import java.util.*;

import javax.imageio.ImageIO;

import io.github.yuvraj0028.bktree.BKTree;
import io.github.yuvraj0028.models.HashType;
import io.github.yuvraj0028.utils.HashUtils;

import java.awt.image.BufferedImage;

/**
 * Service class that handles the core logic for computing, storing, and searching
 * for image similarity using various perceptual hashing algorithms and a BK-Tree.
 * * It manages two main caches: one for the hashes/filenames and one for the BK-Trees.
 */
public class ImageSimilarityService {

    // Storage map: Maps a HashType (PHASH, DHASH, etc.) to a map of (HashValue -> FileName).
    // This allows retrieval of the image's name given its hash. EnumMap is used for efficiency.
    private final Map<HashType, Map<Long, String>> store = new EnumMap<>(HashType.class);
    
    // Cache map: Maps a HashType to its corresponding BKTree.
    // This stores the pre-built, searchable tree for each hashing algorithm.
    private final Map<HashType, BKTree> treeCache = new EnumMap<>(HashType.class);

    /**
     * Initializes the service by creating an empty hash-to-filename map
     * for every supported HashType.
     */
    public ImageSimilarityService() {
        for (HashType hashType : HashType.values()) {
            store.put(hashType, new HashMap<>());
        }
    }

    /**
     * Computes the hash for an image, stores the hash-to-filename mapping,
     * and adds the hash to the relevant BK-Tree.
     * * @param imageFile The image file to process.
     * @param hashType The type of perceptual hash to compute (PHASH, DHASH, etc.).
     * @return The computed 64-bit hash value.
     * @throws IOException if the image file cannot be read.
     */
    public long computeAndStore(File imageFile, HashType hashType) throws IOException {
        BufferedImage img = ImageIO.read(imageFile);
        long hashValue = computeHash(img, hashType);

        // 1. Store the hash-to-filename mapping.
        store.get(hashType).put(hashValue, imageFile.getName());

        // 2. Manage the BK-Tree.
        BKTree tree = treeCache.get(hashType);

        if (tree != null) {
            // Tree exists, just add the new hash.
            tree.add(hashValue);
        } else {
            // Tree does not exist (cache miss or first call). Build the tree from all stored hashes.
            tree = new BKTree();
            // Rebuild the tree by adding all existing hashes for this HashType.
            for (Long hash : store.get(hashType).keySet()) {
                tree.add(hash);
            }
            // Cache the newly built tree.
            treeCache.put(hashType, tree);
        }

        return hashValue;
    }

    /**
     * Computes the perceptual hash of an image using the specified hashing algorithm.
     * * @param img The BufferedImage object.
     * @param hashType The type of hash to compute.
     * @return The 64-bit hash value.
     */
    public long computeHash(BufferedImage img, HashType hashType) {
        switch (hashType) {
            case PHASH: return HashUtils.pHash64(img);
            case DHASH: return HashUtils.dHash64(img);
            case BLOCKHASH: return HashUtils.blockHash64(img);
            default: throw new RuntimeException("HashType not supported");
        }
    }

    /**
     * Retrieves the BKTree for the given HashType from the cache or builds it if necessary.
     * This lazy initialization ensures the tree is only built when a search is requested.
     * * @param type The type of hash/tree to retrieve.
     * @return The BKTree instance.
     */
    private BKTree getOrBuildTree(HashType type) {
        BKTree tree = treeCache.get(type);
        if (tree != null) return tree;

        // Tree is not cached, build it from the stored hashes.
        tree = new BKTree();
        for (Long hash : store.get(type).keySet()) {
            tree.add(hash);
        }
        treeCache.put(type, tree);
        return tree;
    }

    /**
     * Searches the BK-Tree for stored hashes that are perceptually similar to the given hash value.
     * * @param hashValue The query hash.
     * @param type The HashType of the query.
     * @param maxDistance The maximum Hamming distance allowed for a match.
     * @return A list of matching hash values (Long).
     */
    public List<Long> findSimilar(long hashValue, HashType type, int maxDistance) {
        BKTree tree = getOrBuildTree(type);
        return tree.search(hashValue, maxDistance);
    }

    /**
     * High-level method to compute the hash of an image file and find similar
     * images in the store, returning a list of their filenames.
     * * @param imageFile The query image file.
     * @param type The HashType to use.
     * @param maxDistance The maximum Hamming distance for similarity.
     * @return A list of filenames of similar images found in the store.
     * @throws IOException if the image file cannot be read.
     */
    public List<String> findSimilar(File imageFile, HashType type, int maxDistance) throws IOException {
        BufferedImage img = ImageIO.read(imageFile);
        long hashValue = computeHash(img, type);

        // 1. Search the BK-Tree for matching hashes.
        List<Long> matches = findSimilar(hashValue, type, maxDistance);

        // 2. Convert the list of matching hashes back to a list of filenames.
        List<String> results = new ArrayList<>();
        Map<Long, String> map = store.get(type);

        for (Long h : matches) {
            String fileName = map.get(h);
            if (fileName != null) {
                // Ensure we only return stored files (e.g., exclude the query file if it was also stored).
                results.add(fileName);
            }
        }

        return results;
    }

    /**
     * Clears all stored data (hashes, filenames) and clears the entire BK-Tree cache.
     * Resets the service to its initial, empty state.
     */
    public void clearAll() {
        store.values().forEach(Map::clear);
        treeCache.clear();
    }

    /**
     * Clears only the cached BK-Trees. The stored hash-to-filename data remains.
     * This forces the trees to be rebuilt on the next search operation.
     */
    public void clearTreeCache() {
        treeCache.clear();
    }
}
