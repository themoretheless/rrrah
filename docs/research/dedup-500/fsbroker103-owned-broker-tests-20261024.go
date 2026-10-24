package fsbroker
import("testing";"os";"path/filepath")
func TestOwnedAddWatchUsesParentStat(t *testing.T){
 dir:=t.TempDir();for _,n:=range []string{"a","b"}{if e:=os.WriteFile(filepath.Join(dir,n),[]byte("payload"),0600);e!=nil{t.Fatal(e)}}
 b,e:=NewFSBroker(DefaultFSConfig());if e!=nil{t.Fatal(e)};defer b.Stop()
 if e=b.AddWatch(dir);e!=nil{t.Fatal(e)}
 st,e:=os.Stat(dir);if e!=nil{t.Fatal(e)};parent:=FromOSInfo(dir,st)
 if b.watchmap.Size()!=1{t.Fatalf("expected observed single entry,got %d",b.watchmap.Size())}
 last:=b.watchmap.GetByPath(filepath.Join(dir,"b"));if last==nil||last.Id!=parent.Id||!last.IsDir(){t.Fatal("parent metadata observation mismatch")}
 if b.watchmap.GetByPath(filepath.Join(dir,"a"))!=nil{t.Fatal("first child unexpectedly retained")}
}
func TestOwnedRemoveWatchPrefixSibling(t *testing.T){
 b,e:=NewFSBroker(DefaultFSConfig());if e!=nil{t.Fatal(e)};defer b.Stop()
 b.watchmap.Set(&FSInfo{Id:1,Path:"/owned/a/file"});b.watchmap.Set(&FSInfo{Id:2,Path:"/owned/ab/file"})
 b.RemoveWatch("/owned/a")
 if b.watchmap.GetById(2)!=nil{t.Fatal("prefix sibling observation mismatch")}
}
