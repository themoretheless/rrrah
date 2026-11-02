// Copyright (c) 2025-2026, Audionut and the autobrr contributors.
// SPDX-License-Identifier: GPL-2.0-or-later

package mediainfo

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
)

type conformanceDoc struct {
	Media struct {
		Track []map[string]any `json:"track"`
	} `json:"media"`
}

func conformanceError(path string, discType string) (bool, error) {
	if strings.EqualFold(strings.TrimSpace(discType), "BDMV") {
		return false, nil
	}

	payload, err := os.ReadFile(path)
	if err != nil {
		return false, fmt.Errorf("mediainfo: read conformance json: %w", err)
	}

	var doc conformanceDoc
	if err := json.Unmarshal(payload, &doc); err != nil {
		return false, fmt.Errorf("mediainfo: parse conformance json: %w", err)
	}

	for _, track := range doc.Media.Track {
		trackType, _ := track["@type"].(string)
		if !strings.EqualFold(strings.TrimSpace(trackType), "General") {
			continue
		}
		extra, ok := track["extra"].(map[string]any)
		if !ok || len(extra) == 0 {
			return false, nil
		}
		return hasConformanceValue(extra["ConformanceErrors"]), nil
	}

	return false, nil
}

func hasConformanceValue(value any) bool {
	switch typed := value.(type) {
	case nil:
		return false
	case string:
		return strings.TrimSpace(typed) != ""
	case bool:
		return typed
	case float64:
		return typed != 0
	case int:
		return typed != 0
	case []any:
		return len(typed) > 0
	case []string:
		return len(typed) > 0
	case map[string]any:
		return len(typed) > 0
	case map[string]string:
		return len(typed) > 0
	default:
		return strings.TrimSpace(fmt.Sprint(typed)) != ""
	}
}
