# Image Similarity

A lightweight Java library to compute image similarity using perceptual hashing algorithms (pHash, dHash, Blockhash) and BK-Tree for fast similarity search.

---

## Features

- Generate 64-bit perceptual hashes for images
- Supports pHash, dHash, and Blockhash
- Fast similarity lookup using BK-Tree to compare millions of images efficiently
- Find similar images by configurable Hamming distance
- Lightweight, no native dependencies, pure Java

---

## Installation

### Maven
```xml
<dependency>
    <groupId>io.github.yuvraj0028</groupId>
    <artifactId>image-similarity</artifactId>
    <version>1.0.0</version>
</dependency>
```

---

### Gradle
```gradle
implementation 'io.github.yuvraj0028:image-similarity:1.0.0'
```

---

## Usage

### Generate and store hash for an image
```java
import io.github.yuvraj0028.service.ImageSimilarityService;
import io.github.yuvraj0028.models.HashType;
```

```java
// PHASH ALGORITHM
ImageSimilarityService service = new ImageSimilarityService();

File image = new File("images/cat1.jpg");
long hash = service.computeAndStore(image, HashType.PHASH);

// hashed image stored in BK-Tree for comparison
System.out.println("Hash: " + hash);

// DHASH ALGORITHM
image = new File("images/cat2.jpg");
hash = service.computeAndStore(image, HashType.DHASH);

// hashed image stored in BK-Tree for comparison
System.out.println("Hash: " + hash);

// BLOCKHASH ALGORITHM
image = new File("images/cat3.jpg");
hash = service.computeAndStore(image, HashType.BLOCKHASH);

// hashed image stored in BK-Tree for comparison
System.out.println("Hash: " + hash);
```

### Get Hamming Distance
```java
import io.github.yuvraj0028.service.ImageSimilarityService;
import io.github.yuvraj0028.utils.Hamming;
import io.github.yuvraj0028.models.HashType;
```

```java
// PHASH ALGORITHM
ImageSimilarityService service = new ImageSimilarityService();

File image1 = new File("images/cat1.jpg");
long hash1 = service.computeAndStore(image1, HashType.PHASH);

File image2 = new File("images/cat2.jpg");
long hash2 = service.computeAndStore(image2, HashType.PHASH);

// Get hamming distance of the hashes
int hammingDistance = Hamming.distanceLong(hash1, hash2);
System.out.println("Hamming distance: " + hammingDistance);
```

### Find similar images
```java
import io.github.yuvraj0028.service.ImageSimilarityService;
import io.github.yuvraj0028.models.HashType;
```

```java
// PHASH ALGORITHM
ImageSimilarityService service = new ImageSimilarityService();

File image1 = new File("images/cat1.jpg");
long hash1 = service.computeAndStore(image1, HashType.PHASH);

// User can define hamming distance until which they want to find similar images
int hammingDistance = 10;

// Find similar images based on hashes using BK-TREE
File input = new File("images/cat3.jpg");
List<String> similar = service.findSimilar(input, HashType.PHASH, hammingDistance);

System.out.println("Similar images: " + similar);
```

---

## How It Works

This library uses perceptual hashing + BK-Tree index to detect similar images efficiently.

### 1. Perceptual Hashing (pHash, dHash, Blockhash)

Instead of hashing raw file bytes, perceptual hashing extracts the visual fingerprint of the image:

| Hash Type | Focus | Best For |
|-----------|--------|------------|
| pHash     | Visual structure + frequency domain | Best accuracy |
| dHash     | Pixel gradient differences | Faster, good for quick matching |
| Blockhash | Image broken into N×N blocks to detect local differences | Good for resized/cropped images |

Small visual changes → small hash difference.  
If two images look alike, their hash Hamming distance will be low.

---

### 2. BK-Tree for Fast Similarity Search

A BK-Tree (Burkhard–Keller Tree) indexes hash values such that similar hashes are closely linked.

- Searching similar hashes is O(log n) instead of comparing with all images
- Efficient for large-scale similarity lookups (10K–10M images)

Example: Searching with a max Hamming distance of 10 returns visually close images.

---

## Real-world Use Cases

1. **Image Deduplication** – Remove visually duplicate images in photo galleries, cloud storage, or backups.
2. **Content Moderation & NSFW Detection** – Detect reposts or soft duplicates in social media or forums.
3. **Reverse Image Search** – Build small-scale reverse image search services or local image search engines.
4. **CDN & Caching Optimization** – Detect if an uploaded image already exists; avoid redundant storage or processing.
5. **Visual Regression Testing** – Compare UI screenshots across builds to catch visual changes.
6. **E-commerce & Catalogue Management** – Detect sellers posting same product using slightly altered photos.

---

## Requirements

- Java 17+
- Works on Windows, Linux, macOS

---

## License

This project is licensed under the Apache 2.0 License.
