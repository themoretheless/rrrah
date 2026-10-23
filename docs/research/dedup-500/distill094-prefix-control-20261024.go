package main
import (
 "encoding/json"
 "os"
 "time"
 "github.com/Siddhant-K-code/distill/pkg/cache"
 "github.com/Siddhant-K-code/distill/pkg/types"
)
func marked(s string) types.Chunk {return types.Chunk{Text:s,Metadata:map[string]interface{}{"cache_control":true}}}
func main() {
 a:=[]types.Chunk{marked("a\x00b")}; b:=[]types.Chunk{marked("a"),marked("b")}
 pa:=cache.PartitionForCacheAwareDedup(a); pb:=cache.PartitionForCacheAwareDedup(b)
 cfg:=cache.DefaultStabilityConfig();cfg.WarmupChecks=2
 v:=cache.NewStabilityValidator(cfg);v.Check("owned",a); issues:=v.Check("owned",b)
 tr:=cache.NewTTLTracker(time.Minute);first:=tr.Touch(pa.PrefixHash);second:=tr.Touch(pb.PrefixHash)
 snap:=tr.Entry(pa.PrefixHash); snap.HitCount=999
 r:=map[string]interface{}{"left_hash":pa.PrefixHash,"right_hash":pb.PrefixHash,"different_chunk_counts":len(a)!=len(b),"same_hash":pa.PrefixHash==pb.PrefixHash,"stability_issues":len(issues),"stability_rate":v.Stats("owned").StabilityRate(),"first_touch_warm":first,"different_prefix_touch_warm":second,"entry_snapshot_isolated":tr.Entry(pa.PrefixHash).HitCount!=999,"hash_hex_length":len(pa.PrefixHash)}
 if !r["same_hash"].(bool) || second!=true || len(issues)!=0 {panic("unexpected control result")}
 json.NewEncoder(os.Stdout).Encode(r)
}
