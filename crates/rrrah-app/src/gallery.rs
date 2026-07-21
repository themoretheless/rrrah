#![allow(dead_code)]
//! Folder gallery model and bounded thumbnail scheduling.
//!
//! The gallery deliberately keeps filesystem work off the winit thread.  The
//! UI owns `GalleryModel`; a worker can consume `ThumbnailJob`s and publish
//! `ThumbnailReady` messages without touching wgpu resources.

use std::path::{Path, PathBuf};

pub const MAX_ITEMS: usize = 10_000;
pub const THUMB_EDGE: u32 = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GalleryItem {
    pub path: PathBuf,
    pub thumbnail: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct GalleryModel {
    pub items: Vec<GalleryItem>,
    pub selected: usize,
}

impl GalleryModel {
    pub fn replace_folder(&mut self, folder: &Path) {
        self.items = scan_folder(folder)
            .into_iter()
            .map(|path| GalleryItem {
                path,
                thumbnail: None,
            })
            .collect();
        self.selected = 0;
    }

    pub fn select(&mut self, index: usize) -> Option<&Path> {
        if index < self.items.len() {
            self.selected = index;
            return Some(&self.items[index].path);
        }
        None
    }

    /// Prioritized jobs: caller should enqueue these before distant items.
    pub fn jobs(&self, center: usize, radius: usize) -> impl Iterator<Item = ThumbnailJob> + '_ {
        let start = center.saturating_sub(radius);
        let end = (center.saturating_add(radius + 1)).min(self.items.len());
        (start..end).map(|index| ThumbnailJob {
            index,
            source: self.items[index].path.clone(),
            edge: THUMB_EDGE,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ThumbnailJob {
    pub index: usize,
    pub source: PathBuf,
    pub edge: u32,
}

#[derive(Debug, Clone)]
pub struct ThumbnailReady {
    pub index: usize,
    /// CPU-side RGBA8 pixels; upload to a persistent texture atlas on UI side.
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "cr2" | "cr3" | "dng"))
}

pub fn scan_folder(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            (metadata.file_type().is_file() && is_supported(&path)).then_some(path)
        })
        .collect::<Vec<_>>();
    paths.sort_by_cached_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    });
    paths.truncate(MAX_ITEMS);
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extension_filter_is_case_insensitive() {
        assert!(is_supported(Path::new("a.CR3")));
        assert!(!is_supported(Path::new("a.jpg")));
    }
    #[test]
    fn jobs_are_bounded_around_selection() {
        let mut m = GalleryModel::default();
        m.items = (0..5)
            .map(|i| GalleryItem {
                path: PathBuf::from(format!("{i}.cr3")),
                thumbnail: None,
            })
            .collect();
        let jobs = m.jobs(2, 1).collect::<Vec<_>>();
        assert_eq!(jobs.len(), 3);
        assert_eq!(jobs[0].index, 1);
    }
}
