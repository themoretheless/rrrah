# Bitwarden JSON Deduplication Summary

## Original File
- Filename: cli-input.json
- Size: 331 bytes (0.00 MB)
- Folders: 0
- Items: 2

## Deduplicated File
- Filename: cli-input_deduplicated.json
- Size: 306 bytes (0.00 MB)
- Folders: 0
- Items: 1

## Results
- Removed 0 duplicate folders (0% reduction)
- Removed 1 duplicate items (50.0% reduction)
- Reduced file size by 25 bytes (7.6% reduction)

## Method
The deduplication was performed using a Python script that:
1. Identified duplicate folders based on folder names
2. Identified duplicate items based on a composite key of name, type, username, and URIs
3. Preserved the first occurrence of each unique entry
4. Maintained references between items and folders

Generated on: 2026-10-14 21:17:18
