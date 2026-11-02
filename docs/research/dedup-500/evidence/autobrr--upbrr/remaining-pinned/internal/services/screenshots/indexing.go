// Copyright (c) 2025-2026, Audionut and the autobrr contributors.
// SPDX-License-Identifier: GPL-2.0-or-later

package screenshots

// Use preservedScreenshotIndex when loading DB-backed selections where the
// stored order is the source of truth until a later explicit reindex step.
func preservedScreenshotIndex(order int) int {
	return order
}
