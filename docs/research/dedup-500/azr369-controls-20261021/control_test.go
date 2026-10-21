package geometry
import("image";"image/color";"testing")
// Observed upstream limitations, not desired behavior assertions.
func TestReviewEmptyCornersPanics(t *testing.T){
 defer func(){if r:=recover();r==nil{t.Fatal("expected observed empty-corner panic")}else{t.Logf("Observed: %v",r)}}()
 DetectCorners(image.NewGray(image.Rect(0,0,16,16)))
}
func TestReviewRotationLosesColumn(t *testing.T){
 src:=image.NewRGBA(image.Rect(0,0,2,3));for y:=0;y<3;y++{for x:=0;x<2;x++{src.Set(x,y,color.RGBA{uint8(x+1),uint8(y+1),17,255})}}
 out:=InPlaceRotation90(src);zero:=0;for y:=out.Bounds().Min.Y;y<out.Bounds().Max.Y;y++{for x:=out.Bounds().Min.X;x<out.Bounds().Max.X;x++{_,_,_,a:=out.At(x,y).RGBA();if a==0{zero++}}}
 if zero!=3{t.Fatalf("observed transparent pixels=%d,want3",zero)}
 t.Log("Original opaque2x3 loses one source column, three output pixels transparent")
}
