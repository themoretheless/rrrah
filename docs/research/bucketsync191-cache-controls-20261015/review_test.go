package bucketsync
import "testing"
func TestReviewRemovedCapacityRemainsCounted(t *testing.T){c:=NewCache(2);c.Add("a",[]byte{1});c.Add("b",[]byte{2});c.Remove("a");c.Add("c",[]byte{3});if len(c.hash)!=1||c.currentEntries!=2{t.Fatalf("map=%d count=%d",len(c.hash),c.currentEntries)};if _,err:=c.Get("b");err==nil{t.Fatal("expected needless eviction")}}
func TestReviewInputAndOutputSlicesAliasCache(t *testing.T){c:=NewCache(1);data:=[]byte{1};c.Add("a",data);data[0]=2;got,e:=c.Get("a");if e!=nil||got[0]!=2{t.Fatal(got,e)};got[0]=3;again,_:=c.Get("a");if again[0]!=3{t.Fatal(again)}}
func TestReviewZeroAndNegativeCapacity(t *testing.T){for _,n:=range []int{0,-1}{c:=NewCache(n);for i:=0;i<4;i++{c.Add(string(rune('a'+i)),[]byte{1})};want:=1;if n<0{want=4};if len(c.hash)!=want{t.Fatalf("capacity%d got%d",n,len(c.hash))}}}
func TestReviewConfigValidationAcceptsInvalidSizes(t *testing.T){c:=Config{CacheSize:-1,ExtentSize:0};if !c.validate(){t.Fatal("original validation rejected")}}
func TestReviewGetPromotesUnderSharedLock(t *testing.T){c:=NewCache(2);c.Add("a",[]byte{1});c.Add("b",[]byte{2});if c.listHead.next.key!="b"{t.Fatal("setup")};c.lock.RLock();got,e:=c.Get("a");c.lock.RUnlock();if e!=nil||got[0]!=1||c.listHead.next.key!="a"{t.Fatal("get did not mutate under shared lock")}}
