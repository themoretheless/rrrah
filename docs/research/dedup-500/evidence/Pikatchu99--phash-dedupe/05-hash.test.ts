import { describe, expect, it } from "vitest";
import { hammingDistance, hashImage } from "../src/hash.js";
import { makeTempDir, writeFlatImage, writeMirrorImage, writeSplitImage } from "./fixtures.js";

describe("hashImage / hammingDistance", () => {
  it("gives identical hashes for the exact same file", async () => {
    const dir = await makeTempDir();
    const path = await writeSplitImage(dir, "a.png");
    const hashA = await hashImage(path);
    const hashB = await hashImage(path);
    expect(hammingDistance(hashA, hashB)).toBe(0);
  });

  it("gives a small distance for a recompressed copy (near-duplicate)", async () => {
    const dir = await makeTempDir();
    const original = await writeSplitImage(dir, "original.png");
    const recompressed = await writeSplitImage(dir, "recompressed.jpg", 40);
    const distance = hammingDistance(await hashImage(original), await hashImage(recompressed));
    expect(distance).toBeLessThanOrEqual(6);
  });

  it("gives a large distance for a genuinely different image (mirrored gradient)", async () => {
    const dir = await makeTempDir();
    const original = await writeSplitImage(dir, "original.png");
    const mirrored = await writeMirrorImage(dir, "mirrored.png");
    const distance = hammingDistance(await hashImage(original), await hashImage(mirrored));
    expect(distance).toBeGreaterThan(20);
  });

  it("documents the flat-image degenerate case: any solid colour hashes to 0", async () => {
    const dir = await makeTempDir();
    const black = await writeFlatImage(dir, "black.png", [0, 0, 0]);
    const white = await writeFlatImage(dir, "white.png", [255, 255, 255]);
    // No horizontal gradient anywhere in a flat image, so dHash can't tell
    // them apart — this is why the CLI's README warns to eyeball threshold
    // ≥7 matches before deleting anything.
    expect(hammingDistance(await hashImage(black), await hashImage(white))).toBe(0);
  });

  it("respects a custom hashSize", async () => {
    const dir = await makeTempDir();
    const path = await writeSplitImage(dir, "a.png");
    const hash = await hashImage(path, { hashSize: 4 });
    expect(hash).toBeLessThan(1n << 16n); // 4x4 = 16 bits
  });
});
