// Copyright (c) 2025-2026, Audionut and the autobrr contributors.
// SPDX-License-Identifier: GPL-2.0-or-later

package trackers

import "github.com/autobrr/upbrr/pkg/api"

// Use freshScreenshotImageIndex when constructing a brand-new ordered slice for
// rendering or host resolution. Use preservedScreenshotImageIndex only when we
// intentionally carry the stored DB order through before a later normalization
// step.
func freshScreenshotImageIndex(images []api.ScreenshotImage) int {
	return len(images)
}

func preservedScreenshotImageIndex(order int) int {
	return order
}
