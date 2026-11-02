// Copyright (c) 2025-2026, Audionut and the autobrr contributors.
// SPDX-License-Identifier: GPL-2.0-or-later

package api

import (
	"fmt"
	"strings"
)

type Logger interface {
	Tracef(format string, args ...any)
	Debugf(format string, args ...any)
	Infof(format string, args ...any)
	Warnf(format string, args ...any)
	Errorf(format string, args ...any)
}

type NopLogger struct{}

func (NopLogger) Tracef(string, ...any) {
	// Intentionally no-op.
}

func (NopLogger) Debugf(string, ...any) {
	// Intentionally no-op.
}

func (NopLogger) Infof(string, ...any) {
	// Intentionally no-op.
}

func (NopLogger) Warnf(string, ...any) {
	// Intentionally no-op.
}

func (NopLogger) Errorf(string, ...any) {
	// Intentionally no-op.
}

func ParseLogLevel(value string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", "info":
		return "info", nil
	case "trace":
		return "trace", nil
	case "debug":
		return "debug", nil
	case "warn", "warning":
		return "warn", nil
	case "error":
		return "error", nil
	default:
		return "", fmt.Errorf("logging: unknown level %q", value)
	}
}
