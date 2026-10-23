package main
import("encoding/json";"os";"math";c "github.com/Siddhant-K-code/distill/pkg/contextlab";t "github.com/Siddhant-K-code/distill/pkg/types")
func chunks()[]t.Chunk{return []t.Chunk{{ID:"low",Score:.1,Embedding:[]float32{1,0}},{ID:"high",Score:.9,Embedding:[]float32{0,1}}}}
func result()*t.ClusterResult{v:=chunks();return &t.ClusterResult{Clusters:[]t.Cluster{{Members:v[:1]},{Members:v[1:]}}}}
func panics(f func())(yes bool){defer func(){if recover()!=nil{yes=true}}();f();return}
func main(){
 all:=c.SelectTopK(result(),2,c.SelectByScore);if all[0].ID!="low"{panic("unsorted expected")}
 one:=c.SelectTopK(result(),1,c.SelectByScore);if one[0].ID!="high"{panic("top1 positive control")}
 neg:=panics(func(){c.SelectTopK(result(),-1,c.SelectByScore)});if !neg{panic("negative k expected panic")}
 missing:=c.CoverageScore(nil,chunks());perfect:=c.CoverageScore(chunks(),chunks());if missing!=0||perfect!=0{panic("coverage control")}
 nan:=c.NewMMR(c.MMRConfig{Lambda:math.NaN(),TargetK:1}).Rerank(chunks());if len(nan)!=0{panic("nan lambda expected empty")}
 normal:=c.NewMMR(c.MMRConfig{Lambda:1,TargetK:1}).Rerank(chunks());if len(normal)!=1||normal[0].ID!="high"{panic("mmr positive control")}
 v:=chunks();c.NewMMR(c.MMRConfig{Lambda:1,TargetK:1}).RerankWithQuery(v,[]float32{1,0});if v[0].Score!=1||v[1].Score!=0{panic("caller score mutation")}
 json.NewEncoder(os.Stdout).Encode(map[string]interface{}{"top_k_all_first":"low","top_k_one_first":"high","negative_k_panics":neg,"empty_selection_coverage":missing,"perfect_selection_coverage":perfect,"nan_lambda_results":len(nan),"normal_lambda_results":len(normal),"query_rerank_mutates_caller_scores":true})
}
