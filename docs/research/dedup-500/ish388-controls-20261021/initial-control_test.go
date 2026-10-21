package ish
import("image";"image/color";"testing";"bytes")
func TestReviewRowStride(t *testing.T){
 img:=image.NewGray(image.Rect(0,0,9,8));for y:=0;y<8;y++{for x:=0;x<9;x++{v:=x*28;if y%2==1{v=224-v};img.SetGray(x,y,color.Gray{Y:uint8(v)})}}
 resized,err:=Resize(img,9,8);if err!=nil{t.Fatal(err)}
 expected:=make([]byte,8);for y:=0;y<8;y++{for x:=0;x<8;x++{if resized.GrayAt(x,y).Y<resized.GrayAt(x+1,y).Y{i:=y*8+x;expected[i/8]|=1<<uint(i%8)}}}
 actual,err:=NewDifferenceHash(8,8).Hash(img);if err!=nil{t.Fatal(err)};if bytes.Equal(actual,expected){t.Fatal("expected observed row indexing mismatch")};t.Logf("actual=%x correct row oracle=%x",actual,expected)
}
func TestReviewSubimage(t *testing.T){
 parent:=image.NewRGBA(image.Rect(0,0,4,4));sub:=parent.SubImage(image.Rect(1,1,3,3)).(*image.RGBA)
 var caught any;func(){defer func(){caught=recover()}();Grayscale(sub)}();if caught==nil{t.Fatal("expected observed panic on strided subimage")};t.Logf("subimage grayscale panic: %v",caught)
}
func TestReviewSmallHash(t *testing.T){
 var caught any;func(){defer func(){caught=recover()}();_,_=NewAverageHash(1,1).Hash(image.NewGray(image.Rect(0,0,1,1)))}();if caught==nil{t.Fatal("expected accepted dimensions yield zero-byte hash panic")};t.Logf("1x1 average hash panic: %v",caught)
}
