package cornerdetect
import "testing"
func TestReviewMixedIntensityFastRings(t *testing.T){
 circle:=[16][2]int{{0,3},{1,3},{2,2},{3,1},{3,0},{3,-1},{2,-2},{1,-3},{0,-3},{-1,-3},{-2,-2},{-3,-1},{-3,0},{-3,1},{-2,2},{-1,3}}
 offsets:=make([]int,16);make_offsets(offsets,7)
 state:=uint32(369);next:=func()uint8{state^=state<<13;state^=state>>17;state^=state<<5;return uint8(state)}
 cases,scores:=0,0
 for sample:=0;sample<4096;sample++{
  center:=next();ring:=[16]uint8{};for i:=range ring{ring[i]=next()}
  if sample%4==0{for j:=0;j<9;j++{ring[(sample+j)%16]=255};center=64}
  if sample%4==1{for j:=0;j<9;j++{ring[(sample+j)%16]=0};center=192}
  best:=-1
  for start:=0;start<16;start++{bright,dark:=256,256;for j:=0;j<9;j++{delta:=int(ring[(start+j)%16])-int(center);if delta<bright{bright=delta};if -delta<dark{dark=-delta}};if bright-1>best{best=bright-1};if dark-1>best{best=dark-1}}
  im:=make([]uint8,49);for i:=range im{im[i]=center};for i,p:=range circle{im[(3+p[1])*7+3+p[0]]=ring[i]}
  for _,threshold:=range []int{0,30,127,254}{got:=fast9_detect(im,7,7,7,threshold);want:=best>=threshold;if (len(got)==1)!=want{t.Fatalf("sample%d threshold%d detected%v expected%v",sample,threshold,got,want)};cases++;if want{score:=fast9_corner_score(im,24,offsets,threshold);if score!=best{t.Fatalf("sample%d threshold%d score%d expected%d",sample,threshold,score,best)};scores++}}
 }
 t.Logf("%d mixed-intensity threshold cases and %d scores agree with direct cyclic contrast oracle",cases,scores)
}
