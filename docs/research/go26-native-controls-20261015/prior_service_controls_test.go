package service

import (
 "bytes"
 "os"
 "path/filepath"
 "testing"
 set "github.com/deckarep/golang-set/v2"
 "github.com/m-manu/go-find-duplicates/fmte"
)

// These tests document counterexamples in unchanged upstream source, not desired behavior.
func TestRrrahCrossExtensionMissAndSameExtensionPositive(t *testing.T) {
 fmte.Off()
 for _, cross := range []bool{false,true} {
  dir := t.TempDir()
  suffix := ".jpg"; if cross { suffix = ".png" }
  for _, name := range []string{"a.jpg","b"+suffix} {
   if err := os.WriteFile(filepath.Join(dir,name),bytes.Repeat([]byte{37},65536),0600); err != nil {t.Fatal(err)}
  }
  groups, count, _, files, err := FindDuplicates([]string{dir},set.NewSet[string](),0,1,true)
  if err != nil || len(files)!=2 {t.Fatalf("scan: files=%d err=%v",len(files),err)}
  n:=0; if groups!=nil {n=groups.Size()}
  expected:=1; if cross {expected=0}
  if n!=expected || count!=int64(expected) {t.Fatalf("cross=%v groups=%d count=%d",cross,n,count)}
 }
}

func TestRrrahUnsampledDifferenceIsFalseDuplicateOnlyInFastMode(t *testing.T) {
 fmte.Off()
 dir:=t.TempDir(); a:=bytes.Repeat([]byte{37},65536); b:=append([]byte(nil),a...); b[12000]^=1
 if bytes.Equal(a,b) {t.Fatal("negative fixture must differ")}
 for name,data:=range map[string][]byte{"a.bin":a,"b.bin":b} {
  if err:=os.WriteFile(filepath.Join(dir,name),data,0600); err!=nil {t.Fatal(err)}
 }
 for _,thorough:=range []bool{false,true} {
  groups,count,_,files,err:=FindDuplicates([]string{dir},set.NewSet[string](),0,1,thorough)
  if err!=nil || len(files)!=2 || groups==nil {t.Fatalf("scan: %v",err)}
  expected:=1; if thorough {expected=0}
  if groups.Size()!=expected || count!=int64(expected) {t.Fatalf("thorough=%v groups=%d count=%d",thorough,groups.Size(),count)}
 }
}

func TestRrrahMissingRootReturnsNoError(t *testing.T) {
 fmte.Off()
 missing:=filepath.Join(t.TempDir(),"missing")
 groups,count,_,files,err:=FindDuplicates([]string{missing},set.NewSet[string](),0,1,true)
 if err!=nil || len(files)!=0 || count!=0 || groups!=nil {t.Fatalf("unexpected result: err=%v files=%d count=%d",err,len(files),count)}
}

func TestRrrahHardlinkIsCountedAsRedundantCopy(t *testing.T) {
 fmte.Off()
 dir:=t.TempDir(); a:=filepath.Join(dir,"a.bin"); b:=filepath.Join(dir,"b.bin")
 if err:=os.WriteFile(a,[]byte("identical physical file"),0600); err!=nil {t.Fatal(err)}
 if err:=os.Link(a,b); err!=nil {t.Fatal(err)}
 ai,_:=os.Stat(a); bi,_:=os.Stat(b); if !os.SameFile(ai,bi) {t.Fatal("fixture is not a physical alias")}
 groups,count,savings,_,err:=FindDuplicates([]string{dir},set.NewSet[string](),0,1,true)
 if err!=nil || groups==nil || groups.Size()!=1 || count!=1 || savings!=ai.Size() {t.Fatalf("groups=%v count=%d savings=%d err=%v",groups,count,savings,err)}
}
