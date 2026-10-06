import { readdir } from "node:fs/promises";
import { join } from "node:path";

const DEFAULT_EXTENSIONS = new Set([".jpg", ".jpeg", ".png", ".webp", ".avif", ".gif", ".tiff", ".bmp"]);

export interface ScanOptions {
  /** Recurse into subdirectories. Defaults to true. */
  recursive?: boolean;
  /** File extensions to include (lowercase, with the leading dot). Defaults to common raster formats. */
  extensions?: Set<string>;
}

/** Lists every image file under `dir`, returning absolute paths. */
export async function scanImages(dir: string, options: ScanOptions = {}): Promise<string[]> {
  const recursive = options.recursive ?? true;
  const extensions = options.extensions ?? DEFAULT_EXTENSIONS;

  const results: string[] = [];
  const entries = await readdir(dir, { withFileTypes: true });

  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (recursive) results.push(...(await scanImages(fullPath, options)));
      continue;
    }
    const ext = entry.name.slice(entry.name.lastIndexOf(".")).toLowerCase();
    if (extensions.has(ext)) results.push(fullPath);
  }

  return results;
}
