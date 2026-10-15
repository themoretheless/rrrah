package phash
import("testing";"reflect")
func TestTrailingHalfOfHashIsDiscarded(t *testing.T){base:=make([]byte,32);expected:=unpackHash(base);for i:=16;i<32;i++{v:=append([]byte(nil),base...);v[i]=255;if !reflect.DeepEqual(unpackHash(v),expected){t.Fatal(i)}}}
func TestEveryRetainedByteChangesKey(t *testing.T){base:=make([]byte,32);expected:=unpackHash(base);for i:=0;i<16;i++{v:=append([]byte(nil),base...);v[i]=255;if reflect.DeepEqual(unpackHash(v),expected){t.Fatal(i)}}}
func TestArbitraryLengthsAreSilentlyAccepted(t *testing.T){for n:=0;n<=64;n++{input:=make([]byte,n);for i:=range input{input[i]=byte(i+1)};out:=unpackHash(input);if len(out)!=4{t.Fatal(n,out)};for word:=0;word<4;word++{var expected uint32;if (word+1)*4<=n{for j:=0;j<4;j++{expected=(expected<<8)|uint32(input[word*4+j])}};if out[word]!=expected{t.Fatal(n,word,out,expected)}}}}
func TestKnownBigEndianWords(t *testing.T){got:=unpackHash([]byte{0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15});if !reflect.DeepEqual(got,[]uint32{0x00010203,0x04050607,0x08090a0b,0x0c0d0e0f}){t.Fatal(got)}}
