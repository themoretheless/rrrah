#!/usr/bin/env python3
"""AI Smart Photo Dedup - Intelligent duplicate photo cleaner"""

import os
import sys
import argparse
from pathlib import Path
from collections import defaultdict
import imagehash
from PIL import Image
import cv2
import numpy as np
from tqdm import tqdm


class PhotoDeduplicator:
    """AI-powered duplicate photo detector and cleaner."""
    
    def __init__(self, hash_size=16, similarity_threshold=0.9):
        self.hash_size = hash_size
        self.similarity_threshold = similarity_threshold
        self.duplicates = defaultdict(list)
    
    def compute_perceptual_hash(self, image_path):
        """Compute perceptual hash for an image."""
        try:
            with Image.open(image_path) as img:
                return imagehash.phash(img, hash_size=self.hash_size)
        except Exception as e:
            print(f"Error processing {image_path}: {e}")
            return None
    
    def compute_difference_hash(self, image_path):
        """Compute difference hash for an image."""
        try:
            with Image.open(image_path) as img:
                return imagehash.dhash(img, hash_size=self.hash_size)
        except Exception as e:
            return None
    
    def find_duplicates(self, directory, recursive=True):
        """Find all duplicate images in a directory."""
        image_extensions = {'.jpg', '.jpeg', '.png', '.gif', '.bmp', '.webp', '.tiff'}
        image_files = []
        
        if recursive:
            for ext in image_extensions:
                image_files.extend(Path(directory).rglob(f'*{ext}'))
                image_files.extend(Path(directory).rglob(f'*{ext.upper()}'))
        else:
            for ext in image_extensions:
                image_files.extend(Path(directory).glob(f'*{ext}'))
                image_files.extend(Path(directory).glob(f'*{ext.upper()}'))
        
        print(f"Found {len(image_files)} images to analyze...")
        
        hashes = {}
        for img_path in tqdm(image_files, desc="Computing hashes"):
            phash = self.compute_perceptual_hash(str(img_path))
            if phash:
                hashes[str(img_path)] = phash
        
        print("\nFinding duplicates...")
        paths = list(hashes.keys())
        for i in range(len(paths)):
            for j in range(i + 1, len(paths)):
                similarity = 1 - (hashes[paths[i]] - hashes[paths[j]]) / (self.hash_size ** 2)
                if similarity >= self.similarity_threshold:
                    self.duplicates[paths[i]].append(paths[j])
        
        return dict(self.duplicates)
    
    def generate_report(self):
        """Generate a report of found duplicates."""
        total_duplicates = sum(len(v) for v in self.duplicates.values())
        report = {
            'total_images_with_duplicates': len(self.duplicates),
            'total_duplicate_images': total_duplicates,
            'duplicates': dict(self.duplicates)
        }
        return report


def main():
    parser = argparse.ArgumentParser(description='AI Smart Photo Dedup')
    parser.add_argument('directory', help='Directory to scan for duplicates')
    parser.add_argument('-t', '--threshold', type=float, default=0.9,
                        help='Similarity threshold (0-1, default: 0.9)')
    parser.add_argument('-r', '--recursive', action='store_true', default=True,
                        help='Scan directories recursively')
    parser.add_argument('--dry-run', action='store_true',
                        help='Show duplicates without deleting')
    args = parser.parse_args()
    
    dedup = PhotoDeduplicator(similarity_threshold=args.threshold)
    duplicates = dedup.find_duplicates(args.directory, args.recursive)
    
    print(f"\nFound {len(duplicates)} images with duplicates!")
    for original, dupes in duplicates.items():
        print(f"\nOriginal: {original}")
        for d in dupes:
            print(f"  - Duplicate: {d}")


if __name__ == '__main__':
    main()
