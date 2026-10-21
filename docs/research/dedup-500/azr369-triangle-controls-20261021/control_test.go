package triangle
import("image";"image/color";"testing")
// Assertions describe observed upstream defects, not desired behavior.
func TestReviewThreeIdenticalTrianglesRemainDuplicated(t *testing.T){a:=Triangle{{0,0},{1,0},{0,1}};got:=Unique(Triangles{a,a,a});if len(got)!=2{t.Fatalf("observed length=%d",len(got))};t.Log("Unique of three identical triangles returns two")}
func TestReviewComparatorBothDirections(t *testing.T){a:=Triangle{{0,0},{10,0},{20,0}};b:=Triangle{{1,0},{2,0},{3,0}};ts:=Triangles{a,b};if !ts.Less(0,1)||!ts.Less(1,0){t.Fatal("observed comparator changed")};t.Log("Less(a,b) and Less(b,a) are both true")}
func TestReviewTranslatedExtractSamplesOutsideSource(t *testing.T){src:=image.NewRGBA(image.Rect(100,50,145,95));for y:=50;y<95;y++{for x:=100;x<145;x++{src.Set(x,y,color.RGBA{21,43,65,255})}};a:=Triangle{{100,50},{122,88},{145,50}};out:=a.ExtractEquilateralTriangleFrom(src);_,_,_,alpha:=out.At(22,10).RGBA();if alpha!=0{t.Fatalf("observed alpha=%d",alpha)};if !eTriangle.Contains(22,10){t.Fatal("control point outside target triangle")};t.Log("Interior destination sample is transparent despite opaque translated source")}
