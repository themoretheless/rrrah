import {getPref,getFileHash} from './utils.mjs';import {checkForDuplicate} from './duplicateDetector.mjs';import {STATE,createFileRecord} from './trackingStore.mjs';
export async function ownedPolicy(filePath,hash){const lastSyncedSize=3,lastSyncedMtime=2000,canonicalCollectionKey=null;
      const duplicateCheckEnabled = getPref('duplicateCheck') !== false;
      if (duplicateCheckEnabled) {
        try {
          const duplicateResult = await checkForDuplicate({}, filePath);
          if (duplicateResult.isDuplicate) {
            const action = getPref('duplicateAction') || 'skip';
            if (action === 'skip') {
              Zotero.debug(`[WatchFolder] Duplicate detected (${duplicateResult.reason}), skipping: ${filePath}`);
              const existing = duplicateResult.existingItem;
              if (this._trackingStore && existing) {
                // BUG #29 fix: `existing` is typically the PARENT item
                // (Extra-field hash stamps live on the parent), so
                // `existing.key` is the parent's key — NOT the attachment's.
                // Walk the parent's attachments and find the one whose
                // file actually has the matching hash. If `existing` is
                // already an attachment, use it directly.
                let attachment = null;
                let parentKey = null;
                if (existing.isAttachment && existing.isAttachment()) {
                  attachment = existing;
                  parentKey = existing.parentItem?.key ?? null;
                } else {
                  // Parent item — walk children to find the matching attachment.
                  // WP-A4 (perf): batch the attachment fetch (array form of
                  // Zotero.Items.getAsync), then iterate with cached hashes.
                  // Path + hash still need per-item awaits because we break
                  // on first match, but the cache (WP-A1) makes the hash
                  // step itself near-free when re-scanning the same files.
                  parentKey = existing.key;
                  const attIDs = (typeof existing.getAttachments === 'function')
                    ? (existing.getAttachments() || []) : [];
                  let atts = [];
                  if (attIDs.length > 0) {
                    try {
                      const fetched = await Zotero.Items.getAsync(attIDs);
                      atts = Array.isArray(fetched) ? fetched : [];
                    } catch (_e) { atts = []; }
                    if (atts.length === 0) {
                      // Fallback: per-id sync (legacy Zotero.Items.get).
                      atts = attIDs.map(aid => Zotero.Items.get(aid));
                    }
                  }
                  for (const att of atts) {
                    if (!att || !att.isAttachment || !att.isAttachment()) continue;
                    let attPath = null;
                    try { attPath = await att.getFilePathAsync(); }
                    catch (_e) { continue; }
                    if (!attPath) continue;
                    const attHash = await getFileHash(attPath);
                    if (attHash === hash) { attachment = att; break; }
                  }
                  // Fallback: if no child attachment hash matched (e.g. file
                  // sync pending), use the first attachment if there's
                  // exactly one. Otherwise we can't safely pick.
                  if (!attachment && attIDs.length === 1) {
                    const sole = atts[0] ?? Zotero.Items.get(attIDs[0]);
                    if (sole?.isAttachment?.()) attachment = sole;
                  }
                }
                if (attachment) {
                  const watchPath = getPref('sourcePath') || '';
                  const relFile = this._toRelativeForStore(filePath, watchPath);
                  // Don't double-track: if a record for this attachment
                  // at this exact path already exists, skip the insert.
                  const existing = this._trackingStore.getByAttachmentKey(attachment.key);
                  if (existing && this._resolveTrackedAbs(existing.localPath, watchPath) === filePath) {
                    Zotero.debug(`[WatchFolder] Dedup-skip (Extra): existing record for ${attachment.key} already at ${filePath}`);
                    return;
                  }
                  this._trackingStore.add(createFileRecord({
                    localPath: relFile,
                    canonicalLocalPath: relFile,
                    lastSyncedHash: hash,
                    lastSyncedSize,
                    lastSyncedMtime,
                    zoteroItemKey: parentKey,
                    zoteroAttachmentKey: attachment.key,
                    canonicalCollectionKey,
                    collectionMembershipKeys: canonicalCollectionKey ? [canonicalCollectionKey] : [],
                    state: STATE.CLEAN,
                  }));
                  await this._trackingStore.save();
                  return; // skip the import — duplicate handled
                }
                // Couldn't resolve the matching attachment — better to
                // log + fall through to normal import than to store a
                // record with the WRONG attachment key.
                Zotero.debug(`[WatchFolder] Dedup-skip: couldn't resolve attachment for parent ${existing.key} (children=${(existing.getAttachments?.() || []).length}); falling through to import.`);
              } else {
                // No tracking-store or no existing item — preserve legacy
                // skip behavior (no record, no import).
                return;
              }
            }
            Zotero.debug(`[WatchFolder] Duplicate detected but importing anyway (action: ${action}): ${filePath}`);
          }
        } catch (dupError) {
          Zotero.debug(`[WatchFolder] Duplicate check error: ${dupError.message}`);
        }
      }


globalThis.ownedImportCalls++;}
