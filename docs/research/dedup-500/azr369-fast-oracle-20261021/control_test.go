package cornerdetect
import "testing"
func TestReviewAllBinaryFastRings(t *testing.T){
 circle:=[16][2]int{{0,3},{1,3},{2,2},{3,1},{3,0},{3,-1},{2,-2},{1,-3},{0,-3},{-1,-3},{-2,-2},{-3,-1},{-3,0},{-3,1},{-2,2},{-1,3}}
 offsets:=make([]int,16);make_offsets(offsets,7)
 corners:=0
 for mask:=0;mask<65536;mask++{
  im:=make([]uint8,49);for i:=range im{im[i]=128}
  for i,xy:=range circle{value:=uint8(0);if mask&(1<<i)!=0{value=255};im[(3+xy[1])*7+3+xy[0]]=value}
  bright,dark:=false,false
  for start:=0;start<16;start++{ones,zeros:=true,true;for j:=0;j<9;j++{set:=mask&(1<<((start+j)%16))!=0;ones=ones&&set;zeros=zeros&&!set};bright=bright||ones;dark=dark||zeros}
  got:=fast9_detect(im,7,7,7,30);want:=bright||dark
  if (len(got)==1)!=want{t.Fatalf("mask%04x detected%v expected%v",mask,got,want)}
  if want{corners++;score:=fast9_corner_score(im,24,offsets,30);expected:=126;if dark{expected=127};if score!=expected{t.Fatalf("mask%04x score%d expected%d",mask,score,expected)}}
 }
 t.Logf("65536 binary rings matched direct nine-contiguous oracle; %d corner scores matched",corners)
}
