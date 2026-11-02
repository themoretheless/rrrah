// Copyright (c) 2025-2026, Audionut and the autobrr contributors.
// SPDX-License-Identifier: GPL-2.0-or-later

package filesystem

import (
	"context"
	"os"
	"path/filepath"
	"testing"
)

func TestCollectVideoFiles(t *testing.T) {
	tempDir := t.TempDir()

	write := func(path string, size int) {
		data := make([]byte, size)
		if err := os.WriteFile(path, data, 0o600); err != nil {
			t.Fatalf("write file: %v", err)
		}
	}

	t.Run("file path", func(t *testing.T) {
		file := filepath.Join(tempDir, "movie.mkv")
		write(file, 10)
		video, files, err := CollectVideoFiles(context.Background(), file, false)
		if err != nil {
			t.Fatalf("CollectVideoFiles error: %v", err)
		}
		if video != file {
			t.Fatalf("video = %q, want %q", video, file)
		}
		if len(files) != 1 || files[0] != file {
			t.Fatalf("files = %#v", files)
		}
	})

	t.Run("directory with samples", func(t *testing.T) {
		dir := filepath.Join(tempDir, "folder")
		if err := os.MkdirAll(dir, 0o700); err != nil {
			t.Fatalf("mkdir: %v", err)
		}
		write(filepath.Join(dir, "movie.mkv"), 5)
		write(filepath.Join(dir, "movie-sample.mkv"), 8)
		write(filepath.Join(dir, "movie.!sample.mkv"), 7)
		write(filepath.Join(dir, "clip.mp4"), 6)

		video, files, err := CollectVideoFiles(context.Background(), dir, false)
		if err != nil {
			t.Fatalf("CollectVideoFiles error: %v", err)
		}
		if filepath.Base(video) != "clip.mp4" && filepath.Base(video) != "movie.!sample.mkv" && filepath.Base(video) != "movie.mkv" {
			t.Fatalf("unexpected video selection: %q", video)
		}
		for _, file := range files {
			if filepath.Base(file) == "movie-sample.mkv" {
				t.Fatalf("sample file should be excluded")
			}
		}
	})

	t.Run("prefer largest", func(t *testing.T) {
		dir := filepath.Join(tempDir, "largest")
		if err := os.MkdirAll(dir, 0o700); err != nil {
			t.Fatalf("mkdir: %v", err)
		}
		small := filepath.Join(dir, "small.mkv")
		big := filepath.Join(dir, "big.mkv")
		write(small, 1)
		write(big, 20)

		video, files, err := CollectVideoFiles(context.Background(), dir, true)
		if err != nil {
			t.Fatalf("CollectVideoFiles error: %v", err)
		}
		if video != big {
			t.Fatalf("video = %q, want %q", video, big)
		}
		if len(files) != 2 {
			t.Fatalf("files = %#v", files)
		}
	})
}
