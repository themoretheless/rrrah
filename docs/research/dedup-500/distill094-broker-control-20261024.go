package main
import("context";"encoding/json";"os";c "github.com/Siddhant-K-code/distill/pkg/contextlab";t "github.com/Siddhant-K-code/distill/pkg/types")
func chunks()[]t.Chunk{return []t.Chunk{{ID:"a",Score:.1},{ID:"b",Score:.9}}}
func panics(f func())(yes bool){defer func(){if recover()!=nil{yes=true}}();f();return}
type ret struct{cancel context.CancelFunc;top int}
func(r *ret)Query(ctx context.Context,req *t.RetrievalRequest)(*t.RetrievalResult,error){r.top=req.TopK;if r.cancel!=nil{r.cancel()};return &t.RetrievalResult{Chunks:chunks()},nil}
func(r *ret)QueryByID(context.Context,string,int,string)(*t.RetrievalResult,error){return &t.RetrievalResult{},nil}
func(r *ret)Close()error{return nil}
func main(){
 cfg:=c.DefaultBrokerConfig();cfg.TargetK=-1;cfg.OverFetchK=0;cfg.EnableMMR=false
 b:=c.NewBroker(nil,cfg);normal:=b.ProcessChunks(chunks());if len(normal.Chunks)!=2||b.GetConfig().TargetK!=8{panic("constructor defaults")}
 b.SetConfig(cfg);negative:=panics(func(){b.ProcessChunks(chunks())});if !negative{panic("setter negative expected panic")}
 ctx,cancel:=context.WithCancel(context.Background());r:=&ret{cancel:cancel};b=c.NewBroker(r,c.DefaultBrokerConfig());req:=&t.RetrievalRequest{QueryEmbedding:[]float32{1},TopK:1};got,err:=b.Retrieve(ctx,req);if err!=nil||len(got.Chunks)!=2||ctx.Err()==nil{panic("postretrieve cancel control")};if req.TopK!=50||!req.IncludeEmbeddings{panic("request mutation")}
 json.NewEncoder(os.Stdout).Encode(map[string]interface{}{"constructor_negative_target_defaults_to":8,"same_config_setter_panics":negative,"postretrieval_cancel_returns_items":len(got.Chunks),"postretrieval_cancel_returns_nil_error":err==nil,"caller_topk_after":req.TopK})
}
