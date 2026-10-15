package main

import (
    "crypto/md5"
    "crypto/sha1"
    "crypto/sha256"
    "encoding/hex"
    "os"
    "path/filepath"
    "testing"
)

func resetControlState() {
    config.Keep = true
    config.Merge = false
    config.Recursive = false
    config.Verbose = false
    config.Dryrun = true
    config.Workers = 1
    index = map[string]string{}
    hashed, deletes, moves = 0, 0, 0
    newHash = md5.New
}

func writeControlFile(t *testing.T, directory, name string, contents []byte) string {
    t.Helper()
    filename := filepath.Join(directory, name)
    if err := os.WriteFile(filename, contents, 0600); err != nil { t.Fatal(err) }
    return filename
}

func TestOriginalHashFileMatchesThreeStandardAlgorithms(t *testing.T) {
    resetControlState()
    for _, contents := range [][]byte{nil, []byte("abc"), {0, 255, 13, 10}} {
        filename := writeControlFile(t, t.TempDir(), "input", contents)
        for _, algorithm := range []string{"md5", "sha1", "sha256"} {
            var expected string
            switch algorithm {
            case "md5": newHash = md5.New; sum := md5.Sum(contents); expected = hex.EncodeToString(sum[:])
            case "sha1": newHash = sha1.New; sum := sha1.Sum(contents); expected = hex.EncodeToString(sum[:])
            case "sha256": newHash = sha256.New; sum := sha256.Sum256(contents); expected = hex.EncodeToString(sum[:])
            }
            if actual := hashFile(filename); actual != expected { t.Fatalf("%s: %q != %q", algorithm, actual, expected) }
        }
    }
}

func TestOriginalHashFileIgnoresActualDirectoryReadError(t *testing.T) {
    resetControlState()
    directory := t.TempDir()
    file, err := os.Open(directory)
    if err != nil { t.Fatal(err) }
    _, readError := file.Read(make([]byte, 1))
    file.Close()
    if readError == nil { t.Fatal("fixture must actually refuse directory reads") }
    empty := writeControlFile(t, t.TempDir(), "empty", nil)
    if hashFile(directory) != hashFile(empty) { t.Fatal("expected ignored read error to produce empty digest") }
}

func TestOriginalScanIndexesDistinctContentsWithoutMutation(t *testing.T) {
    resetControlState()
    directory := t.TempDir()
    a := writeControlFile(t, directory, "a", []byte("abc"))
    b := writeControlFile(t, directory, "b", []byte("xyz"))
    scan(directory, directory, []string{directory})
    if hashed != 2 || len(index) != 2 || deletes != 0 || moves != 0 { t.Fatalf("hashed=%d index=%d deletes=%d moves=%d", hashed, len(index), deletes, moves) }
    for _, filename := range []string{a, b} { if _, err := os.Stat(filename); err != nil { t.Fatal(err) } }
}

func TestOriginalScanDuplicatesRemainPresentInKeepMode(t *testing.T) {
    resetControlState()
    directory := t.TempDir()
    a := writeControlFile(t, directory, "a", []byte("same"))
    b := writeControlFile(t, directory, "b", []byte("same"))
    scan(directory, directory, []string{directory})
    if hashed != 2 || len(index) != 1 || deletes != 0 || moves != 0 { t.Fatal("unexpected duplicate keep-mode result") }
    for _, filename := range []string{a, b} { contents, err := os.ReadFile(filename); if err != nil || string(contents) != "same" { t.Fatal("keep mode changed source") } }
}

func TestOriginalScanHiddenAndNestedAdmission(t *testing.T) {
    for _, recursive := range []bool{false, true} {
        resetControlState()
        config.Recursive = recursive
        directory := t.TempDir()
        writeControlFile(t, directory, "visible", []byte("visible"))
        writeControlFile(t, directory, ".hidden", []byte("hidden"))
        nested := filepath.Join(directory, "nested")
        if err := os.Mkdir(nested, 0700); err != nil { t.Fatal(err) }
        writeControlFile(t, nested, "other", []byte("nested"))
        scan(directory, directory, []string{directory})
        expected := uint64(1); if recursive { expected = 2 }
        if hashed != expected || len(index) != int(expected) || deletes != 0 || moves != 0 { t.Fatal("unexpected input admission") }
    }
}

func TestOriginalQueueSingleWorkerConsumesEveryEnqueuedPath(t *testing.T) {
    resetControlState()
    queue := newQueue()
    var seen []string
    queue.de(func(value string) { seen = append(seen, value) })
    queue.en([]string{"a", "b", "c"})
    queue.wait()
    if len(seen) != 3 || seen[0] != "a" || seen[1] != "b" || seen[2] != "c" { t.Fatal(seen) }
}
