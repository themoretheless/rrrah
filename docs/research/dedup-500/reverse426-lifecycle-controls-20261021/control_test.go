package web

import (
 "context"
 "testing"
 "time"
 "imgsearch/internal/search"
)

// This asserts observed upstream lifecycle behavior, not the desired contract.
func TestReviewCloseLeavesSearchContextActive(t *testing.T) {
 s := New(0)
 ctx, cancel := context.WithCancel(context.Background())
 defer cancel()
 s.searches["synthetic-active-search"] = &searchState{results: make(chan search.Result), cancel: cancel, createdAt: time.Now(), lastActivity: time.Now()}
 if err := s.Close(); err != nil { t.Fatal(err) }
 select { case <-s.done: default: t.Fatal("Close did not stop cleanup") }
 select { case <-ctx.Done(): t.Fatal("observed behavior changed: search was cancelled"); default: }
 if len(s.searches) != 1 { t.Fatal("observed behavior changed: search map cleared") }
 t.Log("Close stops cleanup but retains one search and does not cancel its context")
}
