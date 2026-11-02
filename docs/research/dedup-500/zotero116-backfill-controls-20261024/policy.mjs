const resolveSyncRoot=async()=>({libraryID:1});export class OwnedBackfill {
  async _backfillHashesForExistingItems() {
    if (!this._trackingStore) return;
    const records = this._trackingStore.getAllOfType('file');
    if (records.length === 0) return;

    const syncRoot = await resolveSyncRoot().catch(() => null);
    const libraryID = syncRoot?.libraryID ?? Zotero.Libraries.userLibraryID;

    let stamped = 0;
    let skipped = 0;
    for (const record of records) {
      if (!record.zoteroAttachmentKey || !record.lastSyncedHash) {
        skipped++;
        continue;
      }
      try {
        const item = await Zotero.Items.getByLibraryAndKeyAsync(libraryID, record.zoteroAttachmentKey);
        if (!item || item.deleted) {
          skipped++;
          continue;
        }
        // The Extra field only exists on regular bibliographic items. For
        // attachments we walk to the parent. If the attachment is still
        // standalone (recognition hasn't created a parent yet), there's
        // nothing to stamp — skip and let a future backfill run after
        // recognition succeeds catch it.
        let target = item;
        if (item.isAttachment && item.isAttachment()) {
          if (!item.parentID) { skipped++; continue; }
          target = await Zotero.Items.getAsync(item.parentID);
          if (!target) { skipped++; continue; }
        }
        const existingExtra = target.getField('extra') || '';
        if (existingExtra.includes(`watchfolder-hash:${record.lastSyncedHash}`)) {
          skipped++;
          continue;
        }
        const newExtra = existingExtra
          ? `${existingExtra}\nwatchfolder-hash:${record.lastSyncedHash}`
          : `watchfolder-hash:${record.lastSyncedHash}`;
        target.setField('extra', newExtra);
        await target.saveTx();
        stamped++;
      } catch (e) {
        Zotero.debug(`[WatchFolder] Backfill error for attachmentKey ${record.zoteroAttachmentKey}: ${e.message}`);
        skipped++;
      }
    }

    if (stamped > 0) {
      Zotero.debug(`[WatchFolder] Backfill: stamped ${stamped} item(s), skipped ${skipped}`);
    } else {
      Zotero.debug(`[WatchFolder] Backfill: no items needed stamping (skipped ${skipped})`);
    }
  }

  /**
   * One-pass backfill: re-queue every plugin-managed standalone PDF
   * attachment for metadata retrieval. Catches two cases that otherwise
   * leave a permanent orphan with no parent registry item:
   *   - the online recognizer failed/timed out on the original import and
   *     was never retried, and
   *   - the file was imported while `autoRetrieveMetadata` was disabled.
   *
   * Scoped to the tracking store (records this plugin wrote), so it never
   * touches unrelated standalone attachments the user added by hand. Items
   * that already have a parent, or already succeeded (`metadataRetrieved`),
   * are skipped. `queueItem` dedups, so repeated calls are safe.
   *
   * @returns {Promise<{queued: number}>}
   */

}
