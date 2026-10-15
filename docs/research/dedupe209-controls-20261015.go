package main
import("testing";"bytes";"strings";"errors")
type failedWriter struct{}
func(failedWriter)Write(p []byte)(int,error){return 0,errors.New("controlled write failure")}
func TestQueryValuesAreIgnored(t *testing.T){p:=NewURLProcessor(nil,nil,nil);a,_:=p.normalizeURL("https://example.test/a?id=1");b,_:=p.normalizeURL("https://example.test/a?id=2");if a!=b{t.Fatal(a,b)}}
func TestSchemeMismatchedPortsAreRemoved(t *testing.T){p:=NewURLProcessor(nil,nil,nil);for _,u:=range []string{"http://example.test:443/a","https://example.test:80/a"}{a,_:=p.normalizeURL(u);if strings.Contains(a,":443")||strings.Contains(a,":80"){t.Fatal(a)}}}
func TestDecodedPathCollapsesEscapedSlash(t *testing.T){p:=NewURLProcessor(nil,nil,nil);a,_:=p.normalizeURL("https://example.test/a%2Fb");b,_:=p.normalizeURL("https://example.test/a/b");if a!=b{t.Fatal(a,b)}}
func TestUserInfoAndFragmentsIgnored(t *testing.T){p:=NewURLProcessor(nil,nil,nil);a,_:=p.normalizeURL("https://alice:one@example.test/a#one");b,_:=p.normalizeURL("https://bob:two@example.test/a#two");if a!=b{t.Fatal(a,b)}}
func TestFinalFlushErrorLost(t *testing.T){err:=deduplicateURLs(strings.NewReader("https://example.test/a\n"),failedWriter{},NewURLProcessor(nil,nil,nil));if err!=nil{t.Fatal("expected original deferred flush error to be lost",err)}}
func TestScannerErrorReturnsAfterPartialOutput(t *testing.T){var out bytes.Buffer;input:="https://example.test/a\n"+strings.Repeat("x",1024*1024+1)+"\n";err:=deduplicateURLs(strings.NewReader(input),&out,NewURLProcessor(nil,nil,nil));if err==nil||out.String()!="https://example.test/a\n"{t.Fatal(err,out.Len())}}
func TestFirstRepresentationRetained(t *testing.T){var out bytes.Buffer;input:="https://example.test/a?id=1\nhttps://example.test/a?id=2\n";err:=deduplicateURLs(strings.NewReader(input),&out,NewURLProcessor(nil,nil,nil));if err!=nil||out.String()!="https://example.test/a?id=1\n"{t.Fatal(err,out.String())}}
func TestSchemeHostCaseNotCanonicalized(t *testing.T){p:=NewURLProcessor(nil,nil,nil);a,_:=p.normalizeURL("https://EXAMPLE.test/a");b,_:=p.normalizeURL("https://example.test/a");if a==b{t.Fatal("expected original host case difference")}}
