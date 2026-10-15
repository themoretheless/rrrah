package dupefinder
import("testing";"strings";"errors";"io";"time";"path/filepath")
func TestReviewUnterminatedCatalog(t *testing.T){d,e:=ParseCatalogReader(strings.NewReader("hash file"));if e!=nil||d["hash"]!="file"{t.Fatalf("%v %v",d,e)}}
func TestReviewDuplicateHashOverwrites(t *testing.T){d,e:=ParseCatalogReader(strings.NewReader("hash first\nhash second\n"));if e!=nil||len(d)!=1||d["hash"]!="second"{t.Fatalf("%v %v",d,e)}}
func TestReviewHashNotValidated(t *testing.T){d,e:=ParseCatalogReader(strings.NewReader("not-sha256 file\n"));if e!=nil||d["not-sha256"]!="file"{t.Fatalf("%v %v",d,e)}}
type reviewErrorReader struct{}
func(reviewErrorReader)Read([]byte)(int,error){return 0,io.ErrUnexpectedEOF}
func TestReviewReaderErrorPropagates(t *testing.T){_,e:=ParseCatalogReader(reviewErrorReader{});if !errors.Is(e,io.ErrUnexpectedEOF){t.Fatal(e)}}
func TestReviewWorkerWaitsForErrorReceiver(t *testing.T){
 errs:=make(chan error);names:=make(chan string,1);entries:=make(chan fileHash);done:=make(chan struct{})
 names<-filepath.Join(t.TempDir(),"missing");close(names)
 go func(){hashFiles(errs,names,entries);close(done)}()
 premature:=false
 select{case <-done:premature=true;case <-time.After(100*time.Millisecond):}
 var e error
 select{case e=<-errs:case <-time.After(3*time.Second):t.Fatal("no pending error")}
 select{case <-done:case <-time.After(3*time.Second):t.Fatal("worker did not finish after receiving error")}
 if premature||e==nil {t.Fatalf("premature=%v error=%v",premature,e)}
 if _,ok:=<-entries;ok {t.Fatal("unexpected entry")}
}
