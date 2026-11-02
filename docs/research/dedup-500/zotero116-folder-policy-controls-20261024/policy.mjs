import {relativePath,delay} from './utils.mjs';import {hashFile as _hashFileCached} from './_hashCache.mjs';import {STATE} from './trackingStore.mjs';export class OwnedPolicy {
  async _detectFolderRenames(scannedFiles, watchPath) {
    if (!this._trackingStore || !watchPath) return;
    const rawRecords = this._trackingStore.getAllOfType('collection');
    if (rawRecords.length === 0) return;

    // Normalize collection records to ABSOLUTE-path form for this
    // method's scope. v2 spec stores relative paths in localPath, but
    // we compare against on-disk absolute paths everywhere here. The
    // resolver is idempotent so both representations work; we tag each
    // record with an `_absLocalPath` field used throughout this fn.
    const collectionRecords = rawRecords.map(r => ({
      ...r,
      _absLocalPath: this._resolveTrackedAbs(r.localPath, watchPath),
    }));

    // ── 1. Build on-disk dir state (ABSOLUTE paths) ────────────────────
    // Include ALL existing subdirs (even empty ones) — otherwise a tracked
    // collection record for an empty folder would be flagged as missing
    // every scan and trigger spurious "no tracked file hashes — skip" log.
    const onDiskDirs = new Set([watchPath, ...(await this._listSubdirectories(watchPath))]);
    for (const fileInfo of scannedFiles) {
      // WP-A2 (perf): prefer the scanner-provided relativePath when present
      // (avoids recomputing from absPath + watchPath each iteration). Falls
      // back to the legacy compute for callers that didn't pass new-shape
      // entries.
      const rel = fileInfo.relativePath ?? relativePath(fileInfo.path, watchPath);
      if (rel == null || rel === '') continue;
      const parts = rel.split('/');
      parts.pop(); // drop filename
      for (let i = 1; i <= parts.length; i++) {
        onDiskDirs.add(watchPath + '/' + parts.slice(0, i).join('/'));
      }
    }

    // ── 2. Find missing collection records ──────────────────────────────
    const missing = collectionRecords.filter(r =>
      r._absLocalPath && !onDiskDirs.has(r._absLocalPath));
    if (missing.length === 0) return;

    // Sort shallowest first so a parent rename is processed before its
    // children — child records get rewritten by the parent's recursive
    // descendant update and won't need a per-child rename.
    missing.sort((a, b) => a._absLocalPath.split('/').length - b._absLocalPath.split('/').length);

    // WP-A1 (perf): module-level (path, size, mtime) LRU cache — survives
    // across scan cycles, so a re-scan with unchanged disk contents hits
    // the cache instead of re-hashing every candidate. See _hashCache.mjs.
    const hashOf = async (p) => _hashFileCached(p);

    // Index scanned files by their absolute ancestor dirs so candidate
    // matching can do O(1) tail lookups against the absolute-path schema.
    const scannedByAbsDir = new Map(); // absDir → [{absPath}, …]
    for (const fileInfo of scannedFiles) {
      // WP-A2 (perf): use scanner-provided relativePath when available.
      const rel = fileInfo.relativePath ?? relativePath(fileInfo.path, watchPath);
      if (rel == null || rel === '') continue;
      const parts = rel.split('/');
      parts.pop();
      for (let i = 1; i <= parts.length; i++) {
        const absDir = watchPath + '/' + parts.slice(0, i).join('/');
        if (!scannedByAbsDir.has(absDir)) scannedByAbsDir.set(absDir, []);
        scannedByAbsDir.get(absDir).push({ absPath: fileInfo.path });
      }
    }

    // ── 2b. Empty-folder rename candidates (conservative structural match) ─
    //
    // The hash matcher below can't help an EMPTY tracked folder (no files →
    // no hash anchor → it would bail and let _ensureCollectionsForExisting
    // Folders create a brand-new collection). For that case we fall back to
    // an UNAMBIGUOUS structural match: an empty tracked collection that
    // vanished is treated as renamed ONLY when, under the SAME parent dir,
    // there is exactly one such orphan AND exactly one new untracked empty
    // directory. Any ambiguity (≥2 orphans, ≥2 new empty dirs, or a
    // candidate dir that actually holds files) falls through to create-new.
    //
    // `dirsWithFiles` = every on-disk dir that has ≥1 scanned file under it
    // (itself or a descendant); used to reject non-empty candidate dirs.
    const dirsWithFiles = new Set(scannedByAbsDir.keys());
    const trackedDirsAll = new Set(
      collectionRecords.map(r => r._absLocalPath).filter(Boolean),
    );
    const parentOf = (absDir) => {
      const idx = absDir.lastIndexOf('/');
      return idx <= 0 ? '' : absDir.slice(0, idx);
    };
    // New untracked empty dirs grouped by parent: parentAbs → [absDir, …]
    const newEmptyDirsByParent = new Map();
    for (const absDir of onDiskDirs) {
      if (absDir === watchPath) continue;
      if (trackedDirsAll.has(absDir)) continue; // already tracked
      if (dirsWithFiles.has(absDir)) continue;  // holds files → not an empty candidate
      const par = parentOf(absDir);
      if (!newEmptyDirsByParent.has(par)) newEmptyDirsByParent.set(par, []);
      newEmptyDirsByParent.get(par).push(absDir);
    }
    // Empty orphaned (missing) collections grouped by parent. "Empty" =
    // no tracked file (with a hash) resolves to or under the missing path.
    // Computed once up front so the per-orphan 1:1 guard sees an accurate
    // orphan count regardless of loop order.
    const allTrackedFileAbs = this._trackingStore.getAllOfType('file')
      .filter(f => f.lastSyncedHash)
      .map(f => this._resolveTrackedAbs(f.localPath, watchPath));
    const missingHasTrackedFiles = (absPath) => {
      const prefix = absPath + '/';
      return allTrackedFileAbs.some(a => a === absPath || a.startsWith(prefix));
    };
    // Only ACTIVE collection records participate in empty-folder rename
    // matching. Dead records (OUT_OF_SCOPE_SUPPRESSED / USER_DETACHED /
    // CONFLICT_BLOCKED / PAUSED / RECOVERABLE / MISSING) linger in the store
    // forever and would otherwise poison the 1:1 orphan count — a single
    // leftover dead orphan under the same parent silently disables a
    // legitimate empty-folder rename for its siblings, causing the mode-3
    // deletion pass to trash-and-recreate the collection (loses the key).
    const DEAD_COLL_STATES = new Set([
      STATE.OUT_OF_SCOPE_SUPPRESSED, STATE.USER_DETACHED, STATE.CONFLICT_BLOCKED,
      STATE.PAUSED, STATE.RECOVERABLE, STATE.MISSING,
    ]);
    const isActiveColl = (r) => !DEAD_COLL_STATES.has(r.state);
    const emptyMissingByParent = new Map();
    for (const r of missing) {
      if (!isActiveColl(r)) continue; // dead orphans must not poison the count
      if (missingHasTrackedFiles(r._absLocalPath)) continue; // not empty
      const par = parentOf(r._absLocalPath);
      if (!emptyMissingByParent.has(par)) emptyMissingByParent.set(par, []);
      emptyMissingByParent.get(par).push(r);
    }

    // ── 3. For each missing record, find a candidate ─────────────────────
    //
    // Matching is *tail-aware*: a candidate dir is only a valid rename
    // target if files in the old subtree map to files in the candidate's
    // subtree at the SAME relative tail (i.e. the inner structure matches).
    // This is what stops a nested-child candidate (Procedures/AI) from
    // beating a true parent candidate (Procedures) when the renamed
    // folder contains subfolders.
    for (const collRecord of missing) {
      // Skip if a prior iteration's recursive descendant sweep already
      // updated this record.
      if (!this._trackingStore.getCollectionRecord(collRecord.zoteroCollectionKey)) continue;
      const oldPath = collRecord._absLocalPath; // normalized above
      const oldPrefix = oldPath + '/';

      // Re-fetch tracked files each iteration — earlier rename sweeps may
      // have rewritten paths. Resolve each file's localPath to absolute
      // for comparison (post-#25 records are sync-root-relative).
      const trackedFilesNow = this._trackingStore.getAllOfType('file');
      const trackedShape = new Map(); // hash → tail under oldPath
      for (const f of trackedFilesNow) {
        if (!f.lastSyncedHash) continue;
        const fAbs = this._resolveTrackedAbs(f.localPath, watchPath);
        let tail;
        if (fAbs === oldPath) tail = '';
        else if (fAbs.startsWith(oldPrefix)) tail = fAbs.slice(oldPrefix.length);
        else continue;
        trackedShape.set(f.lastSyncedHash, tail);
      }
      if (trackedShape.size === 0) {
        // Empty-folder case: no hash anchor. Try the conservative structural
        // matcher — rename in place ONLY on an unambiguous 1:1 match (exactly
        // one empty orphan AND exactly one new untracked empty dir under the
        // SAME parent). Any ambiguity falls through to create-new behavior.
        const par = parentOf(oldPath);
        const orphans = emptyMissingByParent.get(par) || [];
        const newEmpties = newEmptyDirsByParent.get(par) || [];
        if (isActiveColl(collRecord) && orphans.length === 1 && newEmpties.length === 1
            && orphans[0].zoteroCollectionKey === collRecord.zoteroCollectionKey) {
          const target = newEmpties[0];
          Zotero.debug(`[WatchFolder] Empty-folder rename detected (1:1): ${oldPath} → ${target}`);
          await this._renameTrackedCollection(collRecord, target);
        } else {
          Zotero.debug(`[WatchFolder] Folder ${oldPath} missing but no tracked file hashes under it — skip (empty-folder match ambiguous: ${orphans.length} orphan(s), ${newEmpties.length} new empty dir(s))`);
        }
        continue;
      }

      const trackedDirs = new Set(
        this._trackingStore.getAllOfType('collection')
          .map(r => this._resolveTrackedAbs(r.localPath, watchPath)),
      );
      const candidateDirs = [...onDiskDirs].filter(d =>
        d !== watchPath && !trackedDirs.has(d));

      let best = null;
      let bestScore = 0;
      for (const candDir of candidateDirs) {
        const candPrefix = candDir + '/';
        const candEntries = scannedByAbsDir.get(candDir) || [];
        let score = 0;
        for (const { absPath } of candEntries) {
          let candTail;
          if (absPath === candDir) candTail = '';
          else if (absPath.startsWith(candPrefix)) candTail = absPath.slice(candPrefix.length);
          else continue;
          const h = await hashOf(absPath);
          if (h && trackedShape.get(h) === candTail) score++;
        }
        if (score > bestScore) {
          best = candDir;
          bestScore = score;
        }
      }

      if (best && bestScore >= 1) {
        Zotero.debug(`[WatchFolder] Folder rename detected: ${oldPath} → ${best} (matched files=${bestScore})`);
        await this._renameTrackedCollection(collRecord, best);
      }
    }

    try { await this._trackingStore.save(); } catch (_) {}
  }

  /**
   * Apply a detected folder rename: rename the Zotero collection (same
   * key, new name), then sweep descendant file + collection tracking
   * records to update their localPath to the new prefix.
   *
   * @private
   */
  async _waitForFileStable(filePath, maxAttempts = 3) {
    const STABILITY_DELAY = 1000; // 1 second between checks

    try {
      let previousSize = -1;
      let previousMtime = -1;

      for (let attempt = 0; attempt < maxAttempts; attempt++) {
        // Check if file still exists
        const exists = await IOUtils.exists(filePath);
        if (!exists) {
          return false;
        }

        // Get file info
        const info = await IOUtils.stat(filePath);
        const currentSize = info.size;
        const currentMtime = info.lastModified;

        // Stable only when BOTH size AND mtime match the previous read
        if (currentSize === previousSize && currentMtime === previousMtime) {
          // For PDFs, confirm the bytes look complete (header + EOF) before
          // accepting. Other allowed types (epub, djvu, …) have no cheap
          // universal completeness marker — size+mtime convergence IS the
          // stability signal for them, so don't run the PDF check or they'd
          // never import.
          if (filePath.toLowerCase().endsWith('.pdf')) {
            return await this._looksLikeCompletePdf(filePath, currentSize);
          }
          return true;
        }

        previousSize = currentSize;
        previousMtime = currentMtime;

        // Wait before next check (unless last attempt)
        if (attempt < maxAttempts - 1) {
          await delay(STABILITY_DELAY);
        }
      }

      // Never converged on a stable size+mtime: defer to the next poll.
      Zotero.debug(`[WatchFolder] Stability check: ${filePath} never converged on size+mtime; deferring`);
      return false;

    } catch (e) {
      Zotero.debug(`[WatchFolder] Stability check error: ${e.message}`);
      return false;
    }
  }

  /**
   * Sanity-check that a file's bytes look like a complete PDF: a `%PDF-`
   * header and a `%%EOF` marker within the last 1024 bytes. Uses raw byte
   * reads (NOT SHA-256 hashing — does not touch HASH_VERSION or _hashCache).
   * A transient read failure or vanished file returns false (defer), never
   * throws.
   * @private
   * @param {string} filePath - Path to the file
   * @param {number} size - Stable file size from stat()
   * @returns {Promise<boolean>} True if the bytes look like a complete PDF
   */
  async _looksLikeCompletePdf(filePath, size) {
    const HEADER = [0x25, 0x50, 0x44, 0x46, 0x2d]; // "%PDF-"
    const EOF = [0x25, 0x25, 0x45, 0x4f, 0x46];    // "%%EOF"
    const TAIL_BYTES = 1024;

    try {
      if (!Number.isFinite(size) || size < HEADER.length) {
        return false;
      }

      // Header: first 5 bytes must be "%PDF-"
      const head = await IOUtils.read(filePath, { maxBytes: HEADER.length });
      if (!head || head.length < HEADER.length) {
        return false;
      }
      for (let i = 0; i < HEADER.length; i++) {
        if (head[i] !== HEADER[i]) {
          return false;
        }
      }

      // Tail: last <=1024 bytes must contain "%%EOF"
      const tailLen = Math.min(TAIL_BYTES, size);
      const offset = size - tailLen;
      const tail = await IOUtils.read(filePath, { offset, maxBytes: tailLen });
      if (!tail || tail.length === 0) {
        return false;
      }
      for (let i = 0; i + EOF.length <= tail.length; i++) {
        let match = true;
        for (let j = 0; j < EOF.length; j++) {
          if (tail[i + j] !== EOF[j]) {
            match = false;
            break;
          }
        }
        if (match) {
          return true;
        }
      }
      return false;

    } catch (e) {
      Zotero.debug(`[WatchFolder] PDF completeness check error: ${e.message}`);
      return false;
    }
  }

  /**
   * Handle Zotero notifier events
   * Used to track item deletions and updates
   * @param {string} event - Event type (add, modify, delete, etc.)
   * @param {string} type - Object type (item, collection, etc.)
   * @param {number[]} ids - Affected object IDs
   * @param {object} extraData - Additional event data
   */

}
