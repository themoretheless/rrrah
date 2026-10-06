//! UI-side state for the bottom folder filmstrip.
//!
//! Pure layout/hit-test math lives in `rrrah_gpu`; this module owns the tile
//! list, scroll offset, visibility toggle and a small LRU of uploaded
//! thumbnail textures. The LRU is deliberately separate from the decoded
//! mosaic RAM cache: thumbnails are cheap RGBA8 tiles with their own small
//! entry-count budget.

use std::{collections::HashMap, path::PathBuf};

use rrrah_gpu::{FilmstripTile, FilmstripTileId};

use crate::gallery::FolderTile;

use crate::gallery::SourceStamp;

/// Maximum number of uploaded thumbnail textures kept resident.
pub const THUMB_CACHE_CAPACITY: usize = 128;

/// Count-bounded LRU over uploaded strip textures. Eviction returns the GPU
/// handle so the caller can free the texture; the cache itself stays
/// GPU-agnostic for testability.
#[derive(Debug)]
pub struct ThumbCache {
    entries: HashMap<PathBuf, (FilmstripTileId, u64)>,
    sources: HashMap<PathBuf, (PathBuf, SourceStamp)>,
    clock: u64,
    capacity: usize,
}

impl ThumbCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            sources: HashMap::new(),
            clock: 0,
            capacity: capacity.max(1),
        }
    }

    /// Revalidate on folder refresh, outside the drawing path.
    pub fn invalidate_changed_sources(&mut self) -> Vec<FilmstripTileId> {
        let stale: Vec<_> = self
            .sources
            .iter()
            .filter(|(_, (p, s))| SourceStamp::read(p) != *s)
            .map(|(f, _)| f.clone())
            .collect();
        stale
            .into_iter()
            .filter_map(|f| {
                self.sources.remove(&f);
                self.entries.remove(&f).map(|(id, _)| id)
            })
            .collect()
    }
    #[cfg(test)]
    pub fn insert_source(
        &mut self,
        folder: PathBuf,
        source: PathBuf,
        id: FilmstripTileId,
    ) -> Option<FilmstripTileId> {
        self.insert_decoded_source(folder, source.clone(), SourceStamp::read(&source), id)
    }
    pub fn insert_decoded_source(
        &mut self,
        folder: PathBuf,
        source: PathBuf,
        stamp: SourceStamp,
        id: FilmstripTileId,
    ) -> Option<FilmstripTileId> {
        let evicted = self.insert(folder.clone(), id);
        self.sources.insert(folder, (source, stamp));
        evicted
    }
    pub fn contains(&self, path: &std::path::Path) -> bool {
        self.entries.contains_key(path)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Look up a texture, promoting it in recency.
    pub fn get(&mut self, path: &std::path::Path) -> Option<FilmstripTileId> {
        self.clock = self.clock.wrapping_add(1);
        let entry = self.entries.get_mut(path)?;
        entry.1 = self.clock;
        Some(entry.0)
    }

    /// Return an old texture for residency-pressure eviction before admission.
    pub fn evict_oldest(&mut self) -> Option<FilmstripTileId> {
        let victim = self
            .entries
            .iter()
            .min_by_key(|(_, (_, tick))| *tick)
            .map(|(path, _)| path.clone())?;
        self.sources.remove(&victim);
        self.entries.remove(&victim).map(|(id, _)| id)
    }

    /// Insert a texture. Returns the evicted texture handle, if any, so the
    /// caller can remove it from the GPU.
    pub fn insert(&mut self, path: PathBuf, id: FilmstripTileId) -> Option<FilmstripTileId> {
        self.clock = self.clock.wrapping_add(1);
        if let Some(previous) = self.entries.insert(path, (id, self.clock)) {
            return Some(previous.0);
        }
        if self.entries.len() <= self.capacity {
            return None;
        }
        let victim = self
            .entries
            .iter()
            .min_by_key(|(_, (_, tick))| *tick)
            .map(|(path, _)| path.clone())?;
        self.sources.remove(&victim);
        self.entries.remove(&victim).map(|(id, _)| id)
    }
}

/// Filmstrip model: sibling folder tiles, the highlighted current folder,
/// scroll offset and a dirty flag consumed by the render path.
#[derive(Debug)]
pub struct FolderStrip {
    pub visible: bool,
    pub tiles: Vec<FolderTile>,
    pub current: Option<usize>,
    pub scroll: f32,
    pub dirty: bool,
}

impl Default for FolderStrip {
    fn default() -> Self {
        Self {
            visible: true,
            tiles: Vec::new(),
            current: None,
            scroll: 0.0,
            dirty: true,
        }
    }
}

impl FolderStrip {
    /// Replace the tile list after a folder change, keep the current folder
    /// highlighted and auto-scroll so it stays visible.
    pub fn set_folder(&mut self, tiles: Vec<FolderTile>, folder: &std::path::Path, viewport_width: f32) {
        self.current = tiles.iter().position(|tile| tile.folder == folder);
        self.tiles = tiles;
        if let Some(current) = self.current {
            self.scroll = rrrah_gpu::scroll_to_reveal(current, viewport_width, self.tiles.len(), self.scroll);
        } else {
            self.scroll = 0.0;
        }
        self.dirty = true;
    }

    pub fn scroll_by(&mut self, delta: f32, viewport_width: f32) {
        let max = rrrah_gpu::max_scroll(viewport_width, self.tiles.len());
        let next = (self.scroll + delta).clamp(0.0, max);
        if (next - self.scroll).abs() > f32::EPSILON {
            self.scroll = next;
            self.dirty = true;
        }
    }

    /// Hit-test a physical x coordinate against the tile row.
    pub fn tile_at(&self, x: f32) -> Option<usize> {
        rrrah_gpu::tile_index_at(x, self.scroll, self.tiles.len())
    }

    /// Build the renderer's tile list for the current scroll position.
    /// Cached thumbnails are promoted because they are on screen.
    pub fn build_frame(&self, thumbs: &mut ThumbCache) -> Vec<FilmstripTile> {
        self.tiles
            .iter()
            .enumerate()
            .map(|(index, tile)| FilmstripTile {
                x: rrrah_gpu::tile_x(index, self.scroll),
                texture: thumbs.get(&tile.folder),
                label: tile.folder.file_name().map_or_else(
                    || tile.folder.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                ),
                highlighted: self.current == Some(index),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn id(value: usize) -> FilmstripTileId {
        FilmstripTileId::from_raw_for_test(value)
    }

    #[test]
    fn source_and_external_wal_palette_changes_invalidate_textures() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("textures");
        std::fs::create_dir(&folder).unwrap();
        std::fs::create_dir(dir.path().join("pics")).unwrap();
        let source = folder.join("grid.wal");
        let palette = dir.path().join("pics/colormap.pcx");
        std::fs::write(&source, b"unchanged WAL source").unwrap();
        std::fs::write(&palette, b"first palette").unwrap();
        let mut cache = ThumbCache::new(2);
        cache.insert_source(folder.clone(), source.clone(), id(1));
        assert!(cache.invalidate_changed_sources().is_empty());
        std::fs::write(&palette, b"other palette").unwrap();
        assert_eq!(cache.invalidate_changed_sources(), vec![id(1)]);
        assert!(!cache.contains(&folder));
        cache.insert_source(folder.clone(), source.clone(), id(2));
        std::fs::remove_file(&palette).unwrap();
        assert_eq!(cache.invalidate_changed_sources(), vec![id(2)]);
        cache.insert_source(folder.clone(), source.clone(), id(3));
        std::fs::write(&palette, b"new palette").unwrap();
        assert_eq!(cache.invalidate_changed_sources(), vec![id(3)]);
        cache.insert_source(folder.clone(), source.clone(), id(4));
        std::fs::write(&source, b"changed WAL source").unwrap();
        assert_eq!(cache.invalidate_changed_sources(), vec![id(4)]);
        let decoded_stamp = SourceStamp::read(&source);
        std::fs::write(&palette, b"changed during decoding").unwrap();
        cache.insert_decoded_source(folder.clone(), source.clone(), decoded_stamp, id(8));
        assert_eq!(cache.invalidate_changed_sources(), vec![id(8)]);
        cache.insert_source(folder.clone(), source, id(5));
        assert_eq!(cache.insert(PathBuf::from("another folder"), id(6)), None);
        assert_eq!(cache.insert(PathBuf::from("third folder"), id(7)), Some(id(5)));
        assert!(!cache.sources.contains_key(&folder));
    }

    #[test]
    fn pressure_eviction_promotes_recency_and_clears_source_inventory() {
        let mut cache = ThumbCache::new(3);
        for (name, handle) in [("a", 1), ("b", 2)] {
            cache.insert_decoded_source(
                PathBuf::from(name),
                PathBuf::from("missing"),
                SourceStamp::default(),
                id(handle),
            );
        }
        assert_eq!(cache.get(Path::new("a")), Some(id(1)));
        assert_eq!(cache.evict_oldest(), Some(id(2)));
        assert!(!cache.sources.contains_key(Path::new("b")));
        assert!(!cache.contains(Path::new("b")));
        assert_eq!(cache.evict_oldest(), Some(id(1)));
        assert!(cache.sources.is_empty());
        assert_eq!(cache.evict_oldest(), None);
    }

    #[test]
    fn thumb_cache_evicts_least_recently_used_and_reports_handle() {
        let mut cache = ThumbCache::new(2);
        assert_eq!(cache.insert(PathBuf::from("a"), id(1)), None);
        assert_eq!(cache.insert(PathBuf::from("b"), id(2)), None);
        // Promote "a" so "b" becomes the victim.
        assert_eq!(cache.get(Path::new("a")), Some(id(1)));
        assert_eq!(cache.insert(PathBuf::from("c"), id(3)), Some(id(2)));
        assert!(cache.get(Path::new("b")).is_none());
        assert_eq!(cache.get(Path::new("a")), Some(id(1)));
        assert_eq!(cache.get(Path::new("c")), Some(id(3)));
    }

    #[test]
    fn thumb_cache_replacement_returns_previous_handle_without_eviction() {
        let mut cache = ThumbCache::new(1);
        assert_eq!(cache.insert(PathBuf::from("a"), id(1)), None);
        assert_eq!(cache.insert(PathBuf::from("a"), id(2)), Some(id(1)));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(Path::new("a")), Some(id(2)));
    }

    #[test]
    fn strip_frame_marks_current_and_applies_scroll() {
        let mut strip = FolderStrip::default();
        let tiles = (0..10)
            .map(|index| FolderTile {
                folder: PathBuf::from(format!("folder-{index}")),
                cover: PathBuf::from(format!("folder-{index}/a.dng")),
            })
            .collect();
        strip.set_folder(tiles, Path::new("folder-3"), 400.0);
        assert_eq!(strip.current, Some(3));
        assert!(strip.dirty);
        // Tile 3 at [464, 608] does not fit the 400 viewport unscrolled.
        assert!(
            rrrah_gpu::tile_x(3, strip.scroll) + rrrah_gpu::TILE_WIDTH
                <= 400.0 - rrrah_gpu::STRIP_PADDING + 0.001
        );

        let mut thumbs = ThumbCache::new(4);
        let frame = strip.build_frame(&mut thumbs);
        assert_eq!(frame.len(), 10);
        assert!(frame[3].highlighted);
        assert!(!frame[2].highlighted);
        assert_eq!(frame[3].label, "folder-3");
        assert!(frame.iter().all(|tile| tile.texture.is_none()));
    }

    #[test]
    fn strip_scroll_is_clamped_to_content() {
        let mut strip = FolderStrip::default();
        strip.set_folder(
            (0..3)
                .map(|index| FolderTile {
                    folder: PathBuf::from(format!("f{index}")),
                    cover: PathBuf::from("f0/a.dng"),
                })
                .collect(),
            Path::new("f0"),
            2000.0,
        );
        strip.scroll_by(10_000.0, 2000.0);
        assert!(strip.scroll.abs() < 0.001, "everything fits: no scrollable range");
        strip.scroll_by(-10_000.0, 2000.0);
        assert!(strip.scroll.abs() < 0.001);
    }
}
