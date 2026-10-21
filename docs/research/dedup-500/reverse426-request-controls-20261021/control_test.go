package web

import (
 "bytes"
 "context"
 "encoding/json"
 "image/color"
 "mime/multipart"
 "net/http/httptest"
 "testing"
 "time"
 "imgsearch/internal/testutil"
)

// Observed upstream behavior, not a desired cancellation contract.
func TestReviewPreCancelledUploadStillStartsSearch(t *testing.T) {
 dir:=t.TempDir()
 s:=NewWithBasePath(0,dir)
 defer s.Close()
 var body bytes.Buffer
 form:=multipart.NewWriter(&body)
 part,err:=form.CreateFormFile("image","fixture.jpg"); if err!=nil{t.Fatal(err)}
 if _,err=part.Write(testutil.EncodeJPEG(testutil.SolidColorImage(8,8,color.White)));err!=nil{t.Fatal(err)}
 if err=form.WriteField("dir",dir);err!=nil{t.Fatal(err)}
 if err=form.Close();err!=nil{t.Fatal(err)}
 ctx,cancel:=context.WithCancel(context.Background());cancel()
 req:=httptest.NewRequest("POST","/api/search",&body).WithContext(ctx)
 req.Header.Set("Content-Type",form.FormDataContentType())
 recorder:=httptest.NewRecorder()
 s.handleSearch(recorder,req)
 var response map[string]string
 if err=json.Unmarshal(recorder.Body.Bytes(),&response);err!=nil{t.Fatal(err)}
 id:=response["searchId"];if id==""{t.Fatalf("observed behavior changed: %s",recorder.Body.String())}
 s.searchesMu.RLock();state:=s.searches[id];s.searchesMu.RUnlock()
 if state==nil{t.Fatal("missing registered state")};defer state.cancel()
 timer:=time.NewTimer(2*time.Second);defer timer.Stop()
 done:=false
 for {select {
 case result,open:=<-state.results:
  if !open {if !done{t.Fatal("missing completion")};t.Log("Already-cancelled upload request still registered and completed background search");return};done=done||result.Done
 case <-timer.C:t.Fatal("search did not terminate")
 }}
}
