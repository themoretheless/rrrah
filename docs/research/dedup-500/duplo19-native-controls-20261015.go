package duplo

import (
    "bytes"
    "compress/gzip"
    "encoding/gob"
    "image"
    "testing"
)

func TestDHashDiscardsOrdinalArrangement(t *testing.T) {
    a := image.NewGray(image.Rect(0, 0, 8, 8))
    b := image.NewGray(image.Rect(0, 0, 8, 8))
    for y := 0; y < 8; y++ { a.Pix[y*8+1] = 1; b.Pix[y*8+2] = 1 }
    if bytes.Equal(a.Pix, b.Pix) { t.Fatal("fixture contents must differ") }
    expected := [2]uint64{255, 0}
    if dHash(a) != expected || dHash(b) != expected { t.Fatal("expected packed count of eight positive derivatives") }
}

func chromaControlImage() *image.YCbCr {
    img := image.NewYCbCr(image.Rect(0, 0, 8, 8), image.YCbCrSubsampleRatio444)
    for i := range img.Y { img.Y[i] = 64; img.Cb[i] = 200; img.Cr[i] = 220 }
    return img
}

func TestDHashChromaFirstBitsLostToUint8Averaging(t *testing.T) {
    if actual := dHash(chromaControlImage()); actual != [2]uint64{} { t.Fatal(actual) }
}

func TestHistogramChannelsOverlapInsteadOfOccupying64Bits(t *testing.T) {
    actual, maxima := histogram(chromaControlImage())
    const overlapping = uint64(1)<<8 | uint64(1)<<12 | uint64(1)<<29
    const disjoint = uint64(1)<<8 | uint64(1)<<44 | uint64(1)<<61
    if actual != overlapping || actual == disjoint || actual>>32 != 0 || maxima != [3]float32{1, 1, 1} { t.Fatal(actual, maxima) }
}

func TestFlatImagePopulates49149BucketsDespiteTop40(t *testing.T) {
    hash, _ := CreateHash(image.NewGray(image.Rect(0, 0, 8, 8)))
    store := New()
    store.Add("flat-control", hash)
    references := 0
    for _, bucket := range store.indices { references += len(bucket) }
    if references != 3*(ImageScale*ImageScale-1) || references != 49149 { t.Fatal(references) }
}

func controlGobPayload(t *testing.T, values ...interface{}) []byte {
    t.Helper()
    var buffer bytes.Buffer
    compressed := gzip.NewWriter(&buffer)
    encoder := gob.NewEncoder(compressed)
    for _, value := range values { if err := encoder.Encode(value); err != nil { t.Fatal(err) } }
    if err := compressed.Close(); err != nil { t.Fatal(err) }
    return buffer.Bytes()
}

func TestNegativeSerializedCandidateLengthPanics(t *testing.T) {
    payload := controlGobPayload(t, 3, -1)
    defer func() { if recover() == nil { t.Fatal("expected negative-length allocation panic") } }()
    New().GobDecode(payload)
}

func TestFailedDecodeLeavesExistingStoreInconsistent(t *testing.T) {
    hash, _ := CreateHash(image.NewGray(image.Rect(0, 0, 8, 8)))
    store := New()
    store.Add("keep-control", hash)
    if err := store.GobDecode(controlGobPayload(t, 3, 0)); err == nil { t.Fatal("expected truncated input error") }
    if store.Size() != 0 || !store.Has("keep-control") { t.Fatal("expected changed candidates with old ID map retained") }
}

func TestFutureSerializationVersionAcceptedWithoutValidation(t *testing.T) {
    store := New()
    payload := controlGobPayload(t, 999, 0, map[interface{}]uint32{}, store.indices)
    if err := store.GobDecode(payload); err != nil { t.Fatal(err) }
    if store.Size() != 0 || len(store.IDs()) != 0 { t.Fatal("unexpected empty-store state") }
}
