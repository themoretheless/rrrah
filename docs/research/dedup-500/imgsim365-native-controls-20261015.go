package imgsim

import (
    "image"
    "testing"
)

func TestIndependentAverageOracle256Grids(t *testing.T) {
    state := uint32(173)
    for trial := 0; trial < 256; trial++ {
        img := image.NewGray(image.Rect(0, 0, 8, 8))
        var sum uint32
        for i := range img.Pix {
            state ^= state << 13; state ^= state >> 17; state ^= state << 5
            img.Pix[i] = byte(state)
            sum += uint32(img.Pix[i]) * 257
        }
        average := sum / 64
        var expected Hash
        for i, value := range img.Pix { if uint32(value) * 257 > average { expected |= Hash(1) << i } }
        if actual := AverageHash(img); actual != expected { t.Fatalf("trial%d actual%x expected%x", trial, actual, expected) }
    }
}

func TestIndependentDifferenceOracle256Grids(t *testing.T) {
    state := uint32(729)
    for trial := 0; trial < 256; trial++ {
        img := image.NewGray(image.Rect(0, 0, 9, 8))
        for i := range img.Pix { state ^= state << 13; state ^= state >> 17; state ^= state << 5; img.Pix[i] = byte(state) }
        var expected Hash
        for y := 0; y < 8; y++ {
            for x := 1; x < 9; x++ { if img.Pix[y*9+x] > img.Pix[y*9+x-1] { expected |= Hash(1) << (y*8+x-1) } }
        }
        if actual := DifferenceHash(img); actual != expected { t.Fatalf("trial%d actual%x expected%x", trial, actual, expected) }
    }
}

func TestDistinctFlatColorsAndDimensionsCollide(t *testing.T) {
    black := image.NewGray(image.Rect(0, 0, 1, 1))
    white := image.NewGray(image.Rect(0, 0, 257, 37))
    for i := range white.Pix { white.Pix[i] = 255 }
    for _, hash := range []func(image.Image) Hash{AverageHash, DifferenceHash} {
        if hash(black) != 0 || hash(white) != 0 || Distance(hash(black), hash(white)) != 0 { t.Fatal("expected candidate-only flat collision") }
    }
}

func TestNegativeOriginLosesPixelContent(t *testing.T) {
    original := image.NewGray(image.Rect(0, 0, 8, 8))
    translated := image.NewGray(image.Rect(-8, -8, 0, 0))
    for i := range original.Pix { original.Pix[i] = byte(i * 3); translated.Pix[i] = original.Pix[i] }
    for _, hash := range []func(image.Image) Hash{AverageHash, DifferenceHash} {
        if hash(original) == 0 || hash(translated) != 0 { t.Fatal("expected negative-origin grayscale loop to discard content") }
    }
}

func TestDistanceAndFormatting512IndependentPairs(t *testing.T) {
    state := uint64(173)
    next := func() uint64 { state ^= state << 13; state ^= state >> 7; state ^= state << 17; return state }
    for trial := 0; trial < 512; trial++ {
        a, b := next(), next()
        expected := 0
        for bit := 0; bit < 64; bit++ { if ((a >> bit) & 1) != ((b >> bit) & 1) { expected++ } }
        if Distance(Hash(a), Hash(b)) != expected || len(Hash(a).String()) != 64 { t.Fatal("independent Hamming oracle mismatch") }
    }
}

func TestNilInputPanicsRatherThanTypedRefusal(t *testing.T) {
    for _, hash := range []func(image.Image) Hash{AverageHash, DifferenceHash} {
        func() {
            defer func() { if recover() == nil { t.Fatal("expected nil-input panic") } }()
            hash(nil)
        }()
    }
}
