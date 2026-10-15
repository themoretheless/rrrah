package perceptive
import("testing";"image";"image/color";"math/bits")
func flat391(value uint8)*image.Gray{i:=image.NewGray(image.Rect(0,0,8,8));for y:=0;y<8;y++{for x:=0;x<8;x++{i.SetGray(x,y,color.Gray{Y:value})}};return i}
func TestNilFirstErrorIsOverwritten(t *testing.T){for _,kind:=range []PerceptualHash{Average,Difference}{distance,err:=CompareImages(nil,flat391(0),kind);if err!=nil||distance!=0{t.Fatal(kind,distance,err)};if _,err=CompareImages(flat391(0),nil,kind);err!=ErrNilImage{t.Fatal(kind,err)}}}
func TestFlatBlackWhiteCompareAsZero(t *testing.T){for _,kind:=range []PerceptualHash{Average,Difference}{d,err:=CompareImages(flat391(0),flat391(255),kind);if err!=nil||d!=0{t.Fatal(kind,d,err)}}}
func TestEmptyImageProducesZeroHash(t *testing.T){for _,hash:=range []func(image.Image)(uint64,error){Ahash,Dhash}{v,err:=hash(image.NewGray(image.Rect(0,0,0,0)));if err!=nil||v!=0{t.Fatal(v,err)}}}
func TestTypedNilImagePanics(t *testing.T){defer func(){if recover()==nil{t.Fatal("expected typed nil panic")}}();var img *image.Gray;Ahash(img)}
func TestImageOriginDoesNotChangeHash(t *testing.T){a:=image.NewGray(image.Rect(0,0,16,16));b:=image.NewGray(image.Rect(10,20,26,36));for y:=0;y<16;y++{for x:=0;x<16;x++{v:=color.Gray{Y:uint8((x*17+y*31)%256)};a.SetGray(x,y,v);b.SetGray(x+10,y+20,v)}};for _,hash:=range []func(image.Image)(uint64,error){Ahash,Dhash}{x,e:=hash(a);y,f:=hash(b);if e!=nil||f!=nil||x!=y{t.Fatal(x,y,e,f)}}}
func TestHammingMatches65536ScalarInputs(t *testing.T){for i:=0;i<65536;i++{x:=uint64(i)*uint64(0x9e3779b97f4a7c15);y:=uint64(i)^uint64(0xf0f0f0f0f0f0f0f0);if got,want:=HammingDistance(x,y),bits.OnesCount64(x^y);got!=want{t.Fatal(i,got,want)}}}
