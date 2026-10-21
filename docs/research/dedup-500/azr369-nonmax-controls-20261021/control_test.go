package cornerdetect
import("image";"reflect";"testing")
func TestReviewNonmaxDirectNeighbourOracle(t *testing.T){
 cases:=0
 for mask:=0;mask<512;mask++{for mode:=0;mode<4;mode++{
  points:=[]image.Point{};scores:=[]int{}
  for pos:=0;pos<9;pos++{if mask&(1<<pos)!=0{points=append(points,image.Pt(3+pos%3,3+pos/3));value:=0;switch mode{case 1:value=pos;case 2:value=8-pos;case 3:value=pos%2};scores=append(scores,value)}}
  wantPoints:=[]image.Point{};wantScores:=[]int{}
  for i,p:=range points{keep:=true;for j,q:=range points{if i==j{continue};dx,dy:=p.X-q.X,p.Y-q.Y;if dx>=-1&&dx<=1&&dy>=-1&&dy<=1&&scores[j]>=scores[i]{keep=false;break}};if keep{wantPoints=append(wantPoints,p);wantScores=append(wantScores,scores[i])}}
  got:=nonmax_suppression(points,scores)
  if len(got.corners)!=len(wantPoints)||len(got.scores)!=len(wantScores){t.Fatalf("mask%d mode%d got%v/%v want%v/%v",mask,mode,got.corners,got.scores,wantPoints,wantScores)}
  if len(wantPoints)>0&&(!reflect.DeepEqual(got.corners,wantPoints)||!reflect.DeepEqual(got.scores,wantScores)){t.Fatalf("mask%d mode%d mismatch",mask,mode)}
  cases++
 }}
 t.Logf("%d subset/score cases match direct eight-neighbour >= suppression oracle",cases)
}
