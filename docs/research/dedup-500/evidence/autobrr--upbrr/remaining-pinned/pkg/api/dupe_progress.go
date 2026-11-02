// Copyright (c) 2025-2026, Audionut and the autobrr contributors.
// SPDX-License-Identifier: GPL-2.0-or-later

package api

import "context"

type DupeProgressUpdate struct {
	SourcePath string          `json:"sourcePath"`
	Tracker    string          `json:"tracker"`
	Status     string          `json:"status"`
	Message    string          `json:"message"`
	Completed  int             `json:"completed"`
	Total      int             `json:"total"`
	Result     DupeCheckResult `json:"result"`
}

type DupeProgressReporter func(update DupeProgressUpdate)

type dupeProgressReporterKey struct{}

func WithDupeProgressReporter(ctx context.Context, reporter DupeProgressReporter) context.Context {
	if ctx == nil {
		ctx = context.Background()
	}
	if reporter == nil {
		return ctx
	}
	return context.WithValue(ctx, dupeProgressReporterKey{}, reporter)
}

func EmitDupeProgress(ctx context.Context, update DupeProgressUpdate) {
	if ctx == nil {
		return
	}
	reporter, _ := ctx.Value(dupeProgressReporterKey{}).(DupeProgressReporter)
	if reporter == nil {
		return
	}
	reporter(update)
}
