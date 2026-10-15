package dedup
import("testing";"bytes";"io";"errors")
func encoded(t *testing.T)[]byte{t.Helper();var out bytes.Buffer;w,err:=NewStreamWriter(&out,ModeFixed,512,1024);if err!=nil{t.Fatal(err)};if _,err=w.Write(bytes.Repeat([]byte{42},1024));err!=nil{t.Fatal(err)};if err=w.Close();err!=nil{t.Fatal(err)};return out.Bytes()}
type shortWriter struct{bytes.Buffer}
func(w *shortWriter)Write(p []byte)(int,error){if len(p)>1{p=p[:len(p)/2]};return w.Buffer.Write(p)}
func TestWriteToDropsShortWriterRemainder(t *testing.T){r,err:=NewStreamReader(bytes.NewReader(encoded(t)));if err!=nil{t.Fatal(err)};defer r.Close();var out shortWriter;n,err:=r.WriteTo(&out);if err!=io.EOF||n>=1024||out.Len()!=int(n){t.Fatal(n,err,out.Len())}}
func TestReadThenWriteToDropsBufferedRemainder(t *testing.T){r,err:=NewStreamReader(bytes.NewReader(encoded(t)));if err!=nil{t.Fatal(err)};defer r.Close();head:=make([]byte,10);if _,err=io.ReadFull(r,head);err!=nil{t.Fatal(err)};var out bytes.Buffer;n,err:=r.WriteTo(&out);if err!=io.EOF||n!=512{t.Fatal(n,err)}}
type failedWriter struct{}
func(failedWriter)Write(p []byte)(int,error){return 0,errors.New("controlled output failure")}
func TestHeaderAndEmptyTerminatorErrorsAreLost(t *testing.T){w,err:=NewStreamWriter(failedWriter{},ModeFixed,512,1024);if err!=nil{t.Fatal("expected original ignored header failure",err)};if err=w.Close();err!=nil{t.Fatal("expected ignored empty terminator error",err)}}
func TestZeroBlockSizePanicsBeforePolicyError(t *testing.T){defer func(){if recover()==nil{t.Fatal("expected divide by zero")}}();NewStreamWriter(io.Discard,ModeFixed,0,1024)}
