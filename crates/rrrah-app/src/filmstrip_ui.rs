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

/// Maximum number of uploaded thumbnail textures kept resident.
pub const THUMB_CACHE_CAPACITY: usize = 128;

/// Count-bounded LRU over uploaded strip textures. Eviction returns the GPU
/// handle so the caller can free the texture; the cache itself stays
/// GPU-agnostic for testability.
#[derive(Debug)]
pub struct ThumbCache {
    entries: HashMap<PathBuf, (FilmstripTileId, u64)>,
    clock: u64,
    capacity: usize,
}

impl ThumbCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            clock: 0,
            capacity: capacity.max(1),
        }
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
