package periscope

import (
 "bytes"
 "errors"
 "io"
 "os"
 "path/filepath"
 "testing"

 "github.com/spf13/afero"
)

func controlFs(t *testing.T) afero.Fs {
 t.Helper()
 fs:=afero.NewMemMapFs()
 for _,p:=range []string{"/a/x","/b/y"} { if e:=afero.WriteFile(fs,p,[]byte("same"),0600);e!=nil {t.Fatal(e)} }
 return fs
}

func TestControlStaleOutsideScanAndRemovalGuard(t *testing.T) {
 fs:=controlFs(t);ps,_,_:=newTest(fs)
 check(t,ps.Scan([]string{"/"},&ScanOptions{}))
 check(t,afero.WriteFile(fs,"/a/x",[]byte("diff"),0600))
 check(t,ps.Scan([]string{"/b"},&ScanOptions{}))
 sets,e:=ps.db.AllDuplicates("");check(t,e)
 if len(sets)!=1 || len(sets[0])!=2 {t.Fatalf("expected stale duplicate set: %v",sets)}
 if e:=ps.Rm([]string{"/b/y"},&RmOptions{});e==nil {t.Fatal("expected guard refusal")}
 b,readErr:=afero.ReadFile(fs,"/b/y");check(t,readErr);if string(b)!="same" {t.Fatal("victim changed")}
}

type controlHookFs struct {afero.Fs; beforeRemove func(string); denyOpen string}
func (f *controlHookFs) Remove(p string) error {if f.beforeRemove!=nil {f.beforeRemove(p)};return f.Fs.Remove(p)}
func (f *controlHookFs) Open(p string) (afero.File,error) {if p==f.denyOpen {return nil,os.ErrPermission};return f.Fs.Open(p)}

func TestControlMutationAtRemoveLosesNewContent(t *testing.T) {
 base:=controlFs(t);fs:=&controlHookFs{Fs:base};ps,_,_:=newTest(fs)
 check(t,ps.Scan([]string{"/"},&ScanOptions{}))
 fs.beforeRemove=func(p string) {if p!="/b/y" {t.Fatalf("unexpected remove %s",p)};check(t,afero.WriteFile(base,p,[]byte("unique-new-content"),0600))}
 check(t,ps.Rm([]string{"/b/y"},&RmOptions{}))
 if _,e:=base.Stat("/b/y");!os.IsNotExist(e) {t.Fatal("expected victim removed")}
 b,e:=afero.ReadFile(base,"/a/x");check(t,e);if string(b)!="same" {t.Fatal("keeper changed")}
}

func TestControlKeeperDisappearsAtRemove(t *testing.T) {
 base:=controlFs(t);fs:=&controlHookFs{Fs:base};ps,_,_:=newTest(fs)
 check(t,ps.Scan([]string{"/"},&ScanOptions{}))
 fs.beforeRemove=func(p string) {check(t,base.Remove("/a/x"))}
 check(t,ps.Rm([]string{"/b/y"},&RmOptions{}))
 for _,p:=range []string{"/a/x","/b/y"} {if _,e:=base.Stat(p);!os.IsNotExist(e) {t.Fatal("expected both absent")}}
}

func TestControlWalkFailureClearsPriorCatalogue(t *testing.T) {
 base:=controlFs(t);fs:=&controlHookFs{Fs:base};ps,_,_:=newTest(fs)
 check(t,ps.Scan([]string{"/"},&ScanOptions{}))
 before,e:=ps.db.AllInfos();check(t,e);if len(before)!=2 {t.Fatal("wrong fixture")}
 fs.denyOpen="/"
 check(t,ps.Scan([]string{"/"},&ScanOptions{}))
 after,e:=ps.db.AllInfos();check(t,e);if len(after)!=0 {t.Fatalf("expected catalogue erased: %v",after)}
 for _,p:=range []string{"/a/x","/b/y"} {if _,e:=base.Stat(p);e!=nil {t.Fatal(e)}}
}

type controlShortFile struct {afero.File}
func (f *controlShortFile) Read(b []byte)(int,error) {if len(b)>1 {b=b[:1]};return f.File.Read(b)}
type controlShortFs struct {afero.Fs}
func(f *controlShortFs) Open(p string)(afero.File,error) {v,e:=f.Fs.Open(p);if e!=nil{return nil,e};return &controlShortFile{v},nil}

func TestControlPartialHashDependsOnReadChunking(t *testing.T) {
 fs:=controlFs(t);ps,_,_:=newTest(fs)
 whole,e:=ps.hashPartial("/a/x",nil);check(t,e)
 full,e:=ps.hashFile("/a/x");check(t,e)
 ps.fs=&controlShortFs{fs}
 short,e:=ps.hashPartial("/a/x",nil);check(t,e)
 shortFull,e:=ps.hashFile("/a/x");check(t,e)
 if bytes.Equal(whole,short) {t.Fatal("expected chunk-dependent partial hash")}
 if !bytes.Equal(full,shortFull) {t.Fatal("full streaming digest should agree")}
}

func TestControlRootContainedRejectsExistingCopy(t *testing.T) {
 fs:=controlFs(t);ps,_,_:=newTest(fs);check(t,ps.Scan([]string{"/"},&ScanOptions{}))
 if e:=ps.Rm([]string{"/b/y"},&RmOptions{Contained:[]string{"/"}});e==nil {t.Fatal("expected root containment refusal")}
 if _,e:=fs.Stat("/b/y");e!=nil {t.Fatal(e)}
}

type controlFailWriter struct{}
func(controlFailWriter) Write([]byte)(int,error) {return 0,errors.New("owned injected writer failure")}
func TestControlReportIgnoresWriteFailureExportReportsIt(t *testing.T) {
 ps,_,_:=newTest(controlFs(t));check(t,ps.Scan([]string{"/"},&ScanOptions{}));ps.outStream=controlFailWriter{}
 check(t,ps.Report("",&ReportOptions{}))
 if e:=ps.Export(&ExportOptions{});e==nil {t.Fatal("export should report writer error")}
}

func TestControlForgetEmptyPathPanics(t *testing.T) {
 ps,_,_:=newTest(controlFs(t));defer func(){if recover()==nil {t.Fatal("expected empty-path panic")}}()
 ps.Forget([]string{""},&ForgetOptions{})
}

func TestControlRealHardlinkGuard(t *testing.T) {
 dir:=t.TempDir();a:=filepath.Join(dir,"a");b:=filepath.Join(dir,"b")
 check(t,os.WriteFile(a,[]byte("same"),0600));check(t,os.Link(a,b))
 ps,_,_:=newTest(afero.NewOsFs());ps.outStream=io.Discard;ps.errStream=io.Discard
 resolved,e:=filepath.EvalSymlinks(dir);check(t,e);a=filepath.Join(resolved,"a");b=filepath.Join(resolved,"b")
 check(t,ps.Scan([]string{resolved},&ScanOptions{}))
 if e:=ps.Rm([]string{b},&RmOptions{});e==nil {t.Fatal("hardlink keeper must be refused")}
 if _,e:=os.Stat(a);e!=nil {t.Fatal(e)};if _,e:=os.Stat(b);e!=nil {t.Fatal(e)}
}
