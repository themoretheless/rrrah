package client

import (
 "context"
 "io"
 "net/http"
 "strings"
 "testing"
)

type reviewTransport func(*http.Request)(*http.Response,error)
func (f reviewTransport) RoundTrip(r *http.Request)(*http.Response,error){return f(r)}

func TestReviewSendReturnsApplicationFailureWithoutError(t *testing.T){
 p:=New("synthetic")
 p.httpClient.Transport=reviewTransport(func(r *http.Request)(*http.Response,error){
  if r.URL.Path!="/v3/send" {t.Fatal(r.URL.Path)}
  b,err:=io.ReadAll(r.Body);if err!=nil {t.Fatal(err)}
  if !strings.Contains(string(b),`"item_id":"synthetic-item"`) {t.Fatal(string(b))}
  return &http.Response{StatusCode:200,Body:io.NopCloser(strings.NewReader(`{"status":0,"action_results":[false],"action_errors":["synthetic rejection"]}`)),Header:make(http.Header)},nil
 })
 got,err:=p.Send(context.Background(),[]Action{{Type:"delete",ItemID:"synthetic-item"}})
 if err!=nil || got.Status!=0 || len(got.ActionErrors)!=1 {t.Fatalf("got %+v err %v",got,err)}
}
func TestReviewGetReturnsApplicationFailureWithoutError(t *testing.T){
 p:=New("synthetic")
 p.httpClient.Transport=reviewTransport(func(r *http.Request)(*http.Response,error){
  if r.URL.Path!="/v3/get" {t.Fatal(r.URL.Path)}
  return &http.Response{StatusCode:200,Body:io.NopCloser(strings.NewReader(`{"status":0,"list":{}}`)),Header:make(http.Header)},nil
 })
 got,err:=p.Get(context.Background(),GetParams{})
 if err!=nil || got.Status!=0 {t.Fatalf("got %+v err %v",got,err)}
}
