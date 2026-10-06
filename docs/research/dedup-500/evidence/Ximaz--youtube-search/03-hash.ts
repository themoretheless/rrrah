import phash from 'sharp-phash'

// DCT perceptual hash → 64-char "0"/"1" string (castable to Postgres bit(64)).
// pHash canonicalizes (grayscale + fixed-size DCT) before hashing, so it's
// largely resolution/quality-invariant.
//
// IMPORTANT: this pulls in native sharp, which crashes the Nitro dev server's
// worker thread. Only import it from the standalone worker process (main
// thread) — the web server delegates hashing via image/hashClient.ts.
export async function computePhash(bytes: Buffer): Promise<string> {
  return phash(bytes)
}
