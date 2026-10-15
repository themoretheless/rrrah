package main

import (
 "bufio"
 "strings"
 "testing"
)

func TestInjectedHashCollisionIsDiscarded(t *testing.T) {
 index=[][]uint64{{0,42},{1,42},{2,43}}; deDupe(64)
 if index[0][1]!=42 || index[1][1]!=0 || index[2][1]!=43 {t.Fatal(index)}
}

func TestStableMultiwordDuplicateKeepsFirst(t *testing.T) {
 for _,bits:=range []int{64,128,192,256} {
  index=nil
  for i:=0;i<3;i++ {r:=make([]uint64,1+bits/64);r[0]=uint64(i);for j:=1;j<len(r);j++ {r[j]=uint64(j)};index=append(index,r)}
  deDupe(bits)
  if index[0][1]!=1 || index[1][1]!=0 || index[2][1]!=0 {t.Fatal(bits,index)}
 }
}

func TestScrubPreservesCaseAndRejectsTabs(t *testing.T) {
 blackhole:="0.0.0.0"
 got:=scrubInput("0.0.0.0 EXAMPLE.test example.test # comment",&blackhole,false)
 if len(got)!=2 || got[0]!="EXAMPLE.test" || got[1]!="example.test" {t.Fatal(got)}
 if got:=scrubInput("0.0.0.0\texample.test",&blackhole,false);got!=nil {t.Fatal(got)}
}

func TestScannerRejectsOversizedLine(t *testing.T) {
 scanner:=bufio.NewScanner(strings.NewReader("0.0.0.0 "+strings.Repeat("a",70000)+"\n0.0.0.0 tail.test\n"))
 if scanner.Scan() || scanner.Err()==nil {t.Fatal("expected scanner error before later valid line")}
}

func TestByteReverseDoesNotPreserveUTF8(t *testing.T) {
 if makeReverseString("abc")!="cba" {t.Fatal("ascii")}
 if makeReverseString("é")!=string([]byte{0xa9,0xc3}) {t.Fatal("expected byte reversal")}
}
