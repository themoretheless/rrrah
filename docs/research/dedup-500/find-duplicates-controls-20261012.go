// Independent controls for the pinned upstream; not upstream production code.
package dupfind

import (
 "context"
 "crypto/sha256"
 "hash"
 "os"
 "path/filepath"
 "testing"
)

type constantHash struct{}
func (*constantHash) Write(p []byte) (int,error) {return len(p),nil}
func (*constantHash) Sum(b []byte) []byte {return append(b,0)}
func (*constantHash) Reset() {}
func (*constantHash) Size() int {return 1}
func (*constantHash) BlockSize() int {return 1}

func TestControlConstantHashAdmitsDifferentBytes(t *testing.T) {
 root:=t.TempDir()
 for name,data:=range map[string]string{"a":"AAAA","b":"BBBB"} {
  if err:=os.WriteFile(filepath.Join(root,name),[]byte(data),0600);err!=nil {t.Fatal(err)}
 }
 f:=NewDupFinder(WithRoots(root),WithHashFunc(func() hash.Hash{return &constantHash{}}))
 groups,err:=f.FindDuplicates(context.Background());if err!=nil {t.Fatal(err)}
 if len(groups)!=1||len(groups["00"])!=2 {t.Fatalf("expected hash-only collision group, got %v",groups)}
 safe:=NewDupFinder(WithRoots(root),WithHashFunc(sha256.New))
 normal,err:=safe.FindDuplicates(context.Background());if err!=nil||len(normal)!=0 {t.Fatalf("normal hash control: %v %v",normal,err)}
 t.Log("unequal byte strings grouped when hash provider collides; SHA256 control separates them")
}
func TestControlPreviouslyEmptyFilesAreNotReopened(t *testing.T) {
 root:=t.TempDir();a:=filepath.Join(root,"a");b:=filepath.Join(root,"b")
 for _,p:=range []string{a,b} {if err:=os.WriteFile(p,nil,0600);err!=nil {t.Fatal(err)}}
 ai,err:=os.Stat(a);if err!=nil {t.Fatal(err)};bi,err:=os.Stat(b);if err!=nil {t.Fatal(err)}
 if err:=os.WriteFile(a,[]byte("AAAA"),0600);err!=nil {t.Fatal(err)}
 if err:=os.WriteFile(b,[]byte("BBBB"),0600);err!=nil {t.Fatal(err)}
 f:=NewDupFinder(WithHashFunc(sha256.New))
 ah,err:=f.hashPath(pathWithSize{a,ai.Size()});if err!=nil {t.Fatal(err)}
 bh,err:=f.hashPath(pathWithSize{b,bi.Size()});if err!=nil {t.Fatal(err)}
 if ah!=bh||ah!=f.emptyHash||f.Statistics().FilesOpened!=0 {t.Fatal("expected original empty shortcut")}
 t.Log("original hashPath accepts stale zero-size observations without reopening now-unequal files")
}
func TestControlHardlinksAppearAsDuplicatePaths(t *testing.T) {
 root:=t.TempDir();a:=filepath.Join(root,"a");b:=filepath.Join(root,"alias")
 if err:=os.WriteFile(a,[]byte("content"),0600);err!=nil {t.Fatal(err)}
 if err:=os.Link(a,b);err!=nil {t.Fatal(err)}
 ai,err:=os.Stat(a);if err!=nil {t.Fatal(err)};bi,err:=os.Stat(b);if err!=nil {t.Fatal(err)}
 if !os.SameFile(ai,bi) {t.Fatal("fixture must reference same physical file")}
 f:=NewDupFinder(WithRoots(root),WithHashFunc(sha256.New));groups,err:=f.FindDuplicates(context.Background());if err!=nil {t.Fatal(err)}
 if len(groups)!=1 {t.Fatalf("expected one path-hash group, got %v",groups)}
 for _,paths:=range groups {if len(paths)!=2 {t.Fatal(paths)}}
 t.Log("two hardlink paths are a hash group, without distinct-physical-copy classification")
}
