/**
 * Unit tests for content/collectionWatcher.mjs (v2.1 Phase A1).
 *
 * Covers:
 *   UT-301 register/unregister lifecycle (idempotent, hooks into Zotero.Notifier)
 *   UT-302 collection 'add' → createFolder MirrorAction when under sync root
 *   UT-303 collection 'add' → ignored when not under sync root / virtual / sync-root itself
 *   UT-304 collection 'modify' (rename) → moveFolder MirrorAction
 *   UT-305 collection 'modify' (reparent) → moveFolder MirrorAction
 *   UT-306 collection 'modify' (no path change) → no emit
 *   UT-307 collection 'modify' on untracked under-root collection → createFolder
 *   UT-308 collection 'delete' → deleteFolder iff record exists
 *   UT-309 collection-item event → forwarded to itemMembershipHandler
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock collaborators BEFORE importing the SUT so vi.mock hoists correctly.
vi.mock('../../content/mirrorExecutor.mjs', () => ({
  execute: vi.fn(async () => ({ ok: false, reason: 'mocked' })),
}));
vi.mock('../../content/itemMembershipHandler.mjs', () => ({
  handleCollectionItemEvent: vi.fn(async () => {}),
}));
vi.mock('../../content/baseline.mjs', () => ({
  adoptCollectionSubtree: vi.fn(async () => ({ ok: true, copies: 0, mkdirs: 0, errors: 0 })),
}));

import * as mirrorExecutor from '../../content/mirrorExecutor.mjs';
import { handleCollectionItemEvent } from '../../content/itemMembershipHandler.mjs';
import * as baseline from '../../content/baseline.mjs';
import { start, stop, _isRegistered, __test_setDebounceMs, __test_flush } from '../../content/collectionWatcher.mjs';

// ─── Test fixtures ─────────────────────────────────────────────────────────

const SYNC_ROOT = { id: 100, key: 'ROOT1', name: 'Inbox', libraryID: 1, parentID: null };

function makeCollectionRegistry(collections) {
  const byID = new Map(collections.map((c) => [c.id, c]));
  const byKey = new Map(collections.map((c) => [c.key, c]));
  Zotero.Collections.getByLibraryAndKeyAsync = vi.fn(async (libraryID, key) => {
    const c = byKey.get(key);
    if (!c || c.libraryID !== libraryID) return null;
    return c;
  });
  Zotero.Collections.get = vi.fn((id) => byID.get(id) ?? null);
  Zotero.Collections.getByParent = vi.fn((parentID, libraryID) =>
    collections.filter((c) => c.parentID === parentID && c.libraryID === libraryID),
  );
  return { byID, byKey };
}

function prefStubs(values) {
  Zotero.Prefs.get = vi.fn((fullKey) => {
    const prefix = 'extensions.zotero.watchFolder.';
    if (fullKey.startsWith(prefix)) return values[fullKey.slice(prefix.length)];
    return undefined;
  });
}

function makeStore(collectionRecords = []) {
  const byKey = new Map(collectionRecords.map((r) => [r.zoteroCollectionKey, r]));
  return {
    getCollectionRecord: vi.fn((key) => byKey.get(key) ?? null),
  };
}

function makeCoordinator(store) {
  return { _trackingStore: store };
}

/**
 * Capture the observer object passed to Zotero.Notifier.registerObserver so
 * we can invoke `.notify(...)` directly — mirrors how a real Zotero would
 * fan events out.
 */
function captureObserverID() {
  let captured = null;
  Zotero.Notifier.registerObserver = vi.fn((observer, _topics, _name) => {
    captured = observer;
    return 'observer-1';
  });
  Zotero.Notifier.unregisterObserver = vi.fn();
  return () => captured;
}

beforeEach(() => {
  vi.clearAllMocks();
  Zotero.debug = vi.fn();
  Zotero.logError = vi.fn();
  Zotero.Libraries = { userLibraryID: 1, publicationsLibraryID: 4 };
  // Always make sure the watcher is unregistered between tests.
  stop();
  // WP-C #4: drain the debounce buffer immediately in tests so a single
  // `await observer.notify(...)` settles synchronously (after the next
  // microtask tick rather than after 100ms).
  __test_setDebounceMs(0);
});


it('owned: add remove add coalesces to add remove and loses final add in dispatch order',async()=>{makeCollectionRegistry([SYNC_ROOT]);prefStubs({syncRootCollectionKey:'ROOT1',syncRootLibraryID:1});const get=captureObserverID();start(makeCoordinator(makeStore()));let state=false;handleCollectionItemEvent.mockImplementation(async(event)=>{state=event==='add';});const observer=get();await Promise.all([observer.notify('add','collection-item',['100-500'],{}),observer.notify('remove','collection-item',['100-500'],{}),observer.notify('add','collection-item',['100-500'],{})]);expect(handleCollectionItemEvent.mock.calls.map(c=>c[0])).toEqual(['add','remove']);expect(state).toBe(false);stop();});
