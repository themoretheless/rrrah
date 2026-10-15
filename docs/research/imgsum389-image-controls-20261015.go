package image
import("testing";stdimage "image";"image/color";"os";"path/filepath")
func flat(value uint8)*stdimage.Gray{i:=stdimage.NewGray(stdimage.Rect(0,0,32,32));for y:=0;y<32;y++{for x:=0;x<32;x++{i.SetGray(x,y,color.Gray{Y:value})}};return i}
func TestUnknownHasherPanicsRatherThanReturnsError(t *testing.T){defer func(){if recover()==nil{t.Fatal("expected nil hasher panic")}}();i:=&Image{image:flat(0)};i.Hexdigest(HashType("unknown"),32)}
func TestFlatBlackWhiteHashesEqual(t *testing.T){for _,kind:=range []HashType{HashTypeAvg,HashTypeDiff}{a:=&Image{image:flat(0)};b:=&Image{image:flat(255)};x,err:=a.Hexdigest(kind,32);if err!=nil{t.Fatal(err)};y,err:=b.Hexdigest(kind,32);if err!=nil||x!=y{t.Fatal(kind,err,x,y)}}}
func TestOnePixelHashResolutionReturnsDependencyError(t *testing.T){_,err:=(&Image{image:flat(0)}).Hexdigest(HashTypeAvg,1);if err==nil{t.Fatal("expected resize refusal")};t.Log(err)}
func TestMissingAndCorruptImageReturnErrors(t *testing.T){p:=filepath.Join(t.TempDir(),"bad.jpg");if _,err:=NewImage(p);err==nil{t.Fatal("missing")};if err:=os.WriteFile(p,[]byte("not an image"),0600);err!=nil{t.Fatal(err)};if _,err:=NewImage(p);err==nil{t.Fatal("corrupt")}}
func TestCR2FilenamePatternAcceptsMissingDot(t *testing.T){if !reCanon.MatchString("imageXcr2"){t.Fatal("expected original unescaped dot match")}}
