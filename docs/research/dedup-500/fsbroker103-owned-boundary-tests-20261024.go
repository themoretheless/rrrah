package fsbroker
import "testing"
func TestOwnedSamePathReplacement(t *testing.T){
 m:=NewFSMap();a:=&FSInfo{Id:1,Path:"same"};b:=&FSInfo{Id:2,Path:"same"};m.Set(a);m.Set(b)
 if m.Size()!=2 || m.GetById(1)!=a || m.GetByPath("same")!=b {t.Fatal("replacement observation mismatch")}
 m.DeleteById(1)
 if m.GetById(2)!=b || m.GetByPath("same")!=nil {t.Fatal("old ID deletion observation mismatch")}
}
func TestOwnedMutatedPointerRename(t *testing.T){
 m:=NewFSMap();a:=&FSInfo{Id:1,Path:"old"};m.Set(a);a.Path="new";m.Set(a)
 if m.GetByPath("old")!=a || m.GetByPath("new")!=a {t.Fatal("old alias observation mismatch")}
 m.DeleteById(1)
 if m.GetByPath("old")!=a || m.Size()!=0 {t.Fatal("stale alias observation mismatch")}
}
func TestOwnedStackShallowCopy(t *testing.T){
 s:=NewEventStack();e:=&FSEvent{Type:Write,Path:"old"};s.Push(e);s.List()[0].Path="new"
 if s.Pop().Path!="new" {t.Fatal("shared object observation mismatch")}
}
