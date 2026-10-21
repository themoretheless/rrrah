package exif
import("path/filepath";"testing")
func TestReviewZeroExposure(t *testing.T){
 root:=filepath.Join("..","..","..","reverse426-exif-20261021")
 normal:=Extract(filepath.Join(root,"normal.jpg"));if normal.Error!=""||normal.ExposureTime!="1/60 s"{t.Fatalf("normal fixture invalid: %+v",normal)}
 var caught any;func(){defer func(){caught=recover()}();Extract(filepath.Join(root,"zero_numerator.jpg"))}();if caught==nil{t.Fatal("expected observed divide-by-zero panic")};t.Logf("normal=%s; zero-numerator panic=%v",normal.ExposureTime,caught)
}
