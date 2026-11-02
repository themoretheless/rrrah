// Copyright (c) 2025-2026, Audionut and the autobrr contributors.
// SPDX-License-Identifier: GPL-2.0-or-later

package filesystem

import (
	"context"
	"os"
	"path/filepath"
	"testing"
)

func TestDetectDiscType(t *testing.T) {
	cases := []struct {
		name     string
		setup    func(string) string
		expected string
	}{
		{
			name: "no disc",
			setup: func(root string) string {
				return root
			},
			expected: "",
		},
		{
			name: "bdmv",
			setup: func(root string) string {
				path := filepath.Join(root, "Movie", "BDMV")
				if err := os.MkdirAll(path, 0o700); err != nil {
					t.Fatalf("setup bdmv: %v", err)
				}
				return root
			},
			expected: "BDMV",
		},
		{
			name: "bdmv lowercase",
			setup: func(root string) string {
				path := filepath.Join(root, "Lower", "bdmv")
				if err := os.MkdirAll(path, 0o700); err != nil {
					t.Fatalf("setup bdmv lowercase: %v", err)
				}
				return root
			},
			expected: "BDMV",
		},
		{
			name: "dvd",
			setup: func(root string) string {
				path := filepath.Join(root, "DVD", "VIDEO_TS")
				if err := os.MkdirAll(path, 0o700); err != nil {
					t.Fatalf("setup dvd: %v", err)
				}
				return root
			},
			expected: "DVD",
		},
		{
			name: "hddvd",
			setup: func(root string) string {
				path := filepath.Join(root, "HD", "HVDVD_TS")
				if err := os.MkdirAll(path, 0o700); err != nil {
					t.Fatalf("setup hddvd: %v", err)
				}
				return root
			},
			expected: "HDDVD",
		},
		{
			name: "file path",
			setup: func(root string) string {
				filePath := filepath.Join(root, "sample.mkv")
				if err := os.WriteFile(filePath, []byte("x"), 0o600); err != nil {
					t.Fatalf("setup file: %v", err)
				}
				return filePath
			},
			expected: "",
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			root := tc.setup(t.TempDir())
			got, err := DetectDiscType(context.Background(), root)
			if err != nil {
				t.Fatalf("DetectDiscType error: %v", err)
			}
			if got != tc.expected {
				t.Fatalf("DetectDiscType = %q, want %q", got, tc.expected)
			}
		})
	}
}
