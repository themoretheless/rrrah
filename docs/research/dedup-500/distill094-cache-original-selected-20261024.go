package cache
import("context";"testing";"time")
func TestMemoryCache_GetSet(t *testing.T) {
	cache := NewMemoryCache(Config{
		MaxSize:    100,
		DefaultTTL: time.Hour,
	})
	defer func() { _ = cache.Close() }()

	ctx := context.Background()

	// Test Set and Get
	err := cache.Set(ctx, "key1", []byte("value1"), 0)
	if err != nil {
		t.Fatalf("Set failed: %v", err)
	}

	value, err := cache.Get(ctx, "key1")
	if err != nil {
		t.Fatalf("Get failed: %v", err)
	}
	if string(value) != "value1" {
		t.Errorf("expected 'value1', got '%s'", string(value))
	}

	// Test miss
	_, err = cache.Get(ctx, "nonexistent")
	if err != ErrNotFound {
		t.Errorf("expected ErrNotFound, got %v", err)
	}
}

func TestMemoryCache_Delete(t *testing.T) {
	cache := NewMemoryCache(DefaultConfig())
	defer func() { _ = cache.Close() }()

	ctx := context.Background()

	_ = cache.Set(ctx, "key1", []byte("value1"), 0)

	err := cache.Delete(ctx, "key1")
	if err != nil {
		t.Fatalf("Delete failed: %v", err)
	}

	_, err = cache.Get(ctx, "key1")
	if err != ErrNotFound {
		t.Errorf("expected ErrNotFound after delete, got %v", err)
	}

	// Delete nonexistent key
	err = cache.Delete(ctx, "nonexistent")
	if err != ErrNotFound {
		t.Errorf("expected ErrNotFound for nonexistent key, got %v", err)
	}
}

func TestMemoryCache_Has(t *testing.T) {
	cache := NewMemoryCache(DefaultConfig())
	defer func() { _ = cache.Close() }()

	ctx := context.Background()

	if cache.Has(ctx, "key1") {
		t.Error("expected Has to return false for nonexistent key")
	}

	_ = cache.Set(ctx, "key1", []byte("value1"), 0)

	if !cache.Has(ctx, "key1") {
		t.Error("expected Has to return true for existing key")
	}
}

func TestMemoryCache_Clear(t *testing.T) {
	cache := NewMemoryCache(DefaultConfig())
	defer func() { _ = cache.Close() }()

	ctx := context.Background()

	_ = cache.Set(ctx, "key1", []byte("value1"), 0)
	_ = cache.Set(ctx, "key2", []byte("value2"), 0)
	_ = cache.Set(ctx, "key3", []byte("value3"), 0)

	err := cache.Clear(ctx)
	if err != nil {
		t.Fatalf("Clear failed: %v", err)
	}

	stats := cache.Stats()
	if stats.Size != 0 {
		t.Errorf("expected size 0 after clear, got %d", stats.Size)
	}
}

func TestMemoryCache_TTL(t *testing.T) {
	cache := NewMemoryCache(Config{
		MaxSize:         100,
		CleanupInterval: 10 * time.Millisecond,
	})
	defer func() { _ = cache.Close() }()

	ctx := context.Background()

	// Set with short TTL
	_ = cache.Set(ctx, "key1", []byte("value1"), 50*time.Millisecond)

	// Should exist immediately
	if !cache.Has(ctx, "key1") {
		t.Error("expected key to exist immediately after set")
	}

	// Wait for expiration
	time.Sleep(100 * time.Millisecond)

	// Should be expired
	_, err := cache.Get(ctx, "key1")
	if err != ErrNotFound {
		t.Errorf("expected ErrNotFound after TTL, got %v", err)
	}
}

func TestMemoryCache_LRUEviction(t *testing.T) {
	cache := NewMemoryCache(Config{
		MaxSize:    3,
		DefaultTTL: time.Hour,
	})
	defer func() { _ = cache.Close() }()

	ctx := context.Background()

	// Fill cache
	_ = cache.Set(ctx, "key1", []byte("value1"), 0)
	_ = cache.Set(ctx, "key2", []byte("value2"), 0)
	_ = cache.Set(ctx, "key3", []byte("value3"), 0)

	// Access key1 to make it recently used
	_, _ = cache.Get(ctx, "key1")

	// Add new key, should evict key2 (least recently used)
	_ = cache.Set(ctx, "key4", []byte("value4"), 0)

	// key2 should be evicted
	if cache.Has(ctx, "key2") {
		t.Error("expected key2 to be evicted")
	}

	// key1 should still exist (was accessed)
	if !cache.Has(ctx, "key1") {
		t.Error("expected key1 to still exist")
	}

	stats := cache.Stats()
	if stats.Evictions == 0 {
		t.Error("expected at least one eviction")
	}
}

func TestMemoryCache_Stats(t *testing.T) {
	cache := NewMemoryCache(DefaultConfig())
	defer func() { _ = cache.Close() }()

	ctx := context.Background()

	_ = cache.Set(ctx, "key1", []byte("value1"), 0)
	_, _ = cache.Get(ctx, "key1")
	_, _ = cache.Get(ctx, "nonexistent")

	stats := cache.Stats()

	if stats.Sets != 1 {
		t.Errorf("expected 1 set, got %d", stats.Sets)
	}
	if stats.Hits != 1 {
		t.Errorf("expected 1 hit, got %d", stats.Hits)
	}
	if stats.Misses != 1 {
		t.Errorf("expected 1 miss, got %d", stats.Misses)
	}
	if stats.Size != 1 {
		t.Errorf("expected size 1, got %d", stats.Size)
	}
}

func TestStats_HitRate(t *testing.T) {
	tests := []struct {
		hits   int64
		misses int64
		want   float64
	}{
		{0, 0, 0},
		{10, 0, 100},
		{0, 10, 0},
		{50, 50, 50},
		{75, 25, 75},
	}

	for _, tt := range tests {
		stats := Stats{Hits: tt.hits, Misses: tt.misses}
		got := stats.HitRate()
		if got != tt.want {
			t.Errorf("HitRate(%d, %d) = %f, want %f", tt.hits, tt.misses, got, tt.want)
		}
	}
}

