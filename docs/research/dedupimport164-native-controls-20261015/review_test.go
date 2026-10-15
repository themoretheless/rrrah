package main
import("testing";"io";"go/token";"go/parser")
type reviewBrokenWriter struct{short bool;calls int}
func(w *reviewBrokenWriter)Write(b []byte)(int,error){w.calls++;if w.short{return len(b)-1,nil};return 0,io.ErrClosedPipe}
func TestReviewOutputFailureIsIgnored(t *testing.T){*list=false;*overwrite=false;*diff=false;w:=&reviewBrokenWriter{};if e:=writeOutput(w,[]byte("old"),[]byte("new"),"synthetic.go");e!=nil||w.calls!=1{t.Fatal(e,w.calls)}}
func TestReviewShortWriteIsIgnored(t *testing.T){*list=false;*overwrite=false;*diff=false;w:=&reviewBrokenWriter{short:true};if e:=writeOutput(w,[]byte("old"),[]byte("new"),"synthetic.go");e!=nil||w.calls!=1{t.Fatal(e,w.calls)}}
func TestReviewMappingInitializationIsLost(t *testing.T){m:=MultiFlag{name:"m"};if e:=m.Set("example.invalid/path=alias");e!=nil{t.Fatal(e)};if len(m.m)!=0{t.Fatal("unexpected persisted mapping")};m.m=make(map[string]string);if e:=m.Set("example.invalid/path=alias");e!=nil||m.m["example.invalid/path"]!="alias"{t.Fatal(e,m.m)}}
func TestReviewLiteralNormalizationAndFirstSurvivor(t *testing.T){*strategy="first";f,e:=parser.ParseFile(token.NewFileSet(),"synthetic.go","package p\nimport a `fmt`\nimport b \"fmt\"\n",0);if e!=nil{t.Fatal(e)};got:=markDuplicates(f.Imports);if len(got)!=2||got[0].remove||!got[1].remove||got[1].subsumedBy!=f.Imports[0]{t.Fatal("incorrect first survivor")}}
