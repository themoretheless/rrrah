package main
import("errors";"testing";"github.com/m-manu/go-find-duplicates/entity")
type shortWriter struct { calls int }
func(w *shortWriter)Write(p []byte)(int,error){w.calls++;return len(p)-1,nil}
type failedWriter struct{}
func(w failedWriter)Write(p []byte)(int,error){return 0,errors.New("owned injected write failure")}
func fixture()*entity.DigestToFiles{d:=entity.NewDigestToFiles();k:=entity.FileDigest{FileSize:12,FileExtension:".txt",FileHash:"hash"};d.Set(k,"z.txt");d.Set(k,"a.txt");return d}
func TestReviewAllReportFormatsAcceptShortWrites(t *testing.T){for _,m:=range []string{entity.OutputModeTextFile,entity.OutputModeCsvFile,entity.OutputModeJSON}{t.Run(m,func(t *testing.T){w:=&shortWriter{};if err:=reportDuplicates(fixture(),m,entity.FilePathToMeta{},"owned",w);err!=nil||w.calls!=1{t.Fatalf("err=%v calls=%d",err,w.calls)}})}}
func TestReviewAllReportFormatsPropagateWriteErrors(t *testing.T){for _,m:=range []string{entity.OutputModeTextFile,entity.OutputModeCsvFile,entity.OutputModeJSON}{t.Run(m,func(t *testing.T){if err:=reportDuplicates(fixture(),m,entity.FilePathToMeta{},"owned",failedWriter{});err==nil{t.Fatal("expected propagated error")}})}}
func TestReviewTextReportMutatesStoredPathOrder(t *testing.T){d:=fixture();getReportAsText(d);it:=d.Iterator();if !it.HasNext(){t.Fatal("missing")};_,p:=it.Next();if p[0]!="a.txt"{t.Fatal(p)}}
