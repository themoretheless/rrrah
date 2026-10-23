package main
import("encoding/json";"os";"math";c "github.com/Siddhant-K-code/distill/pkg/contextlab";t "github.com/Siddhant-K-code/distill/pkg/types")
func panics(f func())(yes bool){defer func(){if recover()!=nil{yes=true}}();f();return}
func pair()[]t.Chunk{return []t.Chunk{{ID:"a",Embedding:[]float32{1,0}},{ID:"b",Embedding:[]float32{-1,0}}}}
func chain()[]t.Chunk{v:=[]t.Chunk{};for i:=0;i<3;i++{a:=float64(i)*math.Pi/9;v=append(v,t.Chunk{Embedding:[]float32{float32(math.Cos(a)),float32(math.Sin(a))}})};return v}
func main(){
 cfg:=c.DefaultClusterConfig();base:=c.NewClusterer(cfg).Cluster(pair());if base.ClusterCount!=2{panic("baseline")}
 cfg.Threshold=2;opposite:=panics(func(){c.NewClusterer(cfg).Cluster(pair())});if !opposite{panic("opposite expected panic")}
 cfg.Threshold=math.NaN();nan:=panics(func(){c.NewClusterer(cfg).Cluster(pair())});if !nan{panic("nan expected panic")}
 cfg.Threshold=.1;mismatch:=panics(func(){c.NewClusterer(cfg).Cluster([]t.Chunk{{Embedding:[]float32{1,0}},{Embedding:[]float32{1}}})});if !mismatch{panic("dimension expected panic")}
 cfg.Linkage="single";single:=c.NewClusterer(cfg).Cluster(chain());if single.ClusterCount!=1{panic("single linkage")}
 cfg.Linkage="complete";complete:=c.NewClusterer(cfg).Cluster(chain());if complete.ClusterCount!=2{panic("complete linkage")}
 json.NewEncoder(os.Stdout).Encode(map[string]interface{}{"default_opposites_clusters":2,"threshold_two_opposites_panics":opposite,"nan_threshold_opposites_panics":nan,"mixed_dimension_merge_panics":mismatch,"single_linkage_chain_clusters":1,"complete_linkage_chain_clusters":2})
}
