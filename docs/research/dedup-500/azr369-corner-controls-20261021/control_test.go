package cornerdetect
import("image";"testing")
func TestReviewShiftedGrayBounds(t *testing.T){
 zero:=image.NewGray(image.Rect(0,0,16,16));if got:=Fast9(zero,30);len(got)!=0{t.Fatalf("flat control found %d corners",len(got))}
 defer func(){if r:=recover();r==nil{t.Fatal("observed shifted-bound panic absent")}else{t.Logf("Observed shifted16x16Gray panic: %v",r)}}()
 Fast9(image.NewGray(image.Rect(100,50,116,66)),30)
}
