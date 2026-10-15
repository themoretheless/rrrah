package deduper
import("testing";"errors")
type mutableReq struct{k interface{};p string}
func(r *mutableReq)Key()interface{}{return r.k}
func(r *mutableReq)Payload()string{return r.p}
func manual()*Deduper[int,string]{return &Deduper[int,string]{workerCh:make(chan Request[string],8),resultCh:make(chan *resultWrapper[int,string],8)}}
func wrap(r *mutableReq)*requestWrapper[int,string]{return &requestWrapper[int,string]{r,make(chan *resultWrapper[int,string],1)}}
func TestSameKeyDifferentPayloadSharesFirstResult(t *testing.T){dd:=manual();saved:=make(map[interface{}][]*requestWrapper[int,string]);a:=wrap(&mutableReq{"same","first"});b:=wrap(&mutableReq{"same","second"});dd.queueRequestChan(a,saved);dd.queueRequestChan(b,saved);if len(dd.workerCh)!=1||(<-dd.workerCh).Payload()!="first"{t.Fatal("dispatch")};dd.processResult(&resultWrapper[int,string]{a.request,7,nil},saved);if (<-a.returnCh).value!=7||(<-b.returnCh).value!=7||len(saved)!=0{t.Fatal("fanout")}}
func TestMutableKeyLosesOriginalWaiter(t *testing.T){dd:=manual();saved:=make(map[interface{}][]*requestWrapper[int,string]);r:=&mutableReq{"before","payload"};a:=wrap(r);dd.queueRequestChan(a,saved);r.k="after";dd.processResult(&resultWrapper[int,string]{r,7,nil},saved);if len(a.returnCh)!=0||len(saved["before"])!=1{t.Fatal("expected stranded original waiter")}}
func TestUncomparableKeyPanics(t *testing.T){defer func(){if recover()==nil{t.Fatal("expected map key panic")}}();manual().queueRequestChan(wrap(&mutableReq{[]int{1},"payload"}),make(map[interface{}][]*requestWrapper[int,string]))}
func TestErrorFanoutClearsInflight(t *testing.T){dd:=manual();saved:=make(map[interface{}][]*requestWrapper[int,string]);a:=wrap(&mutableReq{"x","payload"});dd.queueRequestChan(a,saved);err:=errors.New("controlled");dd.processResult(&resultWrapper[int,string]{a.request,0,err},saved);if (<-a.returnCh).err!=err||len(saved)!=0{t.Fatal("error fanout")}}
