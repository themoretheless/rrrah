package main
import("context";"encoding/json";"os";"math";d "github.com/Siddhant-K-code/distill/pkg/dedup";m "github.com/Siddhant-K-code/distill/pkg/math";t "github.com/Siddhant-K-code/distill/pkg/types")
func main(){
 if m.CosineDistance([]float32{1,0},[]float32{1})!=0{panic("dimension control")}
 cfg:=d.DefaultConfig();cfg.K=1;cfg.Seed=94;cfg.Workers=1
 v:=[]t.Vector{{ID:"x1",Values:[]float32{1,0}},{ID:"x2",Values:[]float32{1,0}},{ID:"y1",Values:[]float32{0,1}},{ID:"y2",Values:[]float32{0,1}},{ID:"y3",Values:[]float32{0,1}}}
 result,err:=d.NewEngine(cfg).Deduplicate(context.Background(),v);if err!=nil||len(result.UniqueVectors)!=3{panic("medoid control")}
 cfg.Threshold=math.NaN();bad,err:=d.NewEngine(cfg).Deduplicate(context.Background(),v);if err!=nil||len(bad.UniqueVectors)!=1{panic("NaN control")}
 meta:=map[string]interface{}{"nested":map[string]string{"a":"before"}};original:=t.NewVectorWithMetadata("id",[]float32{1},meta);clone:=original.Clone();clone.Metadata["nested"].(map[string]string)["a"]="after";if original.Metadata["nested"].(map[string]string)["a"]!="after"{panic("clone control")}
 cfg.Threshold=.05;equal,err:=d.NewEngine(cfg).Deduplicate(context.Background(),v[:2]);if err!=nil||len(equal.UniqueVectors)!=1{panic("equal control")}
 json.NewEncoder(os.Stdout).Encode(map[string]interface{}{"different_dimensions_prefix_distance":0,"two_exact_value_classes_kept":len(result.UniqueVectors),"nan_threshold_kept":len(bad.UniqueVectors),"nested_clone_aliases_original":true,"equal_pair_positive_control_kept":1})
}
