// Copyright (c) 2025-2026, Audionut and the autobrr contributors.
// SPDX-License-Identifier: GPL-2.0-or-later

package filesystem

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	internalerrors "github.com/autobrr/upbrr/internal/errors"
)

func TestValidatePaths(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	filePath := filepath.Join(dir, "sample.txt")
	if err := os.WriteFile(filePath, []byte("data"), 0o600); err != nil {
		t.Fatalf("write file: %v", err)
	}

	validator := NewValidator()

	paths, err := validator.ValidatePaths(context.Background(), []string{dir, filePath})
	if err != nil {
		t.Fatalf("expected valid paths, got error: %v", err)
	}
	if len(paths) != 2 {
		t.Fatalf("expected 2 paths, got %d", len(paths))
	}

	_, err = validator.ValidatePaths(context.Background(), []string{""})
	if !errors.Is(err, internalerrors.ErrInvalidInput) {
		t.Fatalf("expected invalid input error, got: %v", err)
	}

	_, err = validator.ValidatePaths(context.Background(), []string{"/does-not-exist"})
	if !errors.Is(err, internalerrors.ErrNotFound) {
		t.Fatalf("expected not found error, got: %v", err)
	}
}
