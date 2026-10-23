package main
import("context";"encoding/json";"os";"time";c "github.com/Siddhant-K-code/distill/pkg/cache")
func main(){
 ctx:=context.Background();cache:=c.NewMemoryCache(c.Config{MaxSize:10,MaxSizeBytes:10,CleanupInterval:time.Hour});defer cache.Close()
 if cache.Set(ctx,"a",[]byte("v"),0)!=nil||cache.Set(ctx,"b",[]byte("v"),0)!=nil{panic("initial")};if cache.Set(ctx,"a",[]byte("12345678"),0)!=nil{panic("replace")};size:=cache.Stats().SizeBytes;if size!=11{panic("expected budget excess")}
 x:=c.NewMemoryCache(c.Config{MaxSize:10,CleanupInterval:time.Hour});defer x.Close();input:=[]byte("v");x.Set(ctx,"key",input,0);input[0]='x';got,_:=x.Get(ctx,"key");if string(got)!="x"{panic("input alias")};got[0]='y';again,_:=x.Get(ctx,"key");if string(again)!="y"{panic("output alias")}
 canceled,cancel:=context.WithCancel(ctx);cancel();err:=x.Set(canceled,"canceled",[]byte("saved"),0);if err!=nil||!x.Has(ctx,"canceled"){panic("canceled write")}
 json.NewEncoder(os.Stdout).Encode(map[string]interface{}{"configured_bytes":10,"after_existing_key_growth_bytes":size,"input_mutation_changes_cache":true,"get_mutation_changes_cache":true,"canceled_write_stored":true})
}
