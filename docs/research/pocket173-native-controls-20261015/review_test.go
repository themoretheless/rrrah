package deduper

import (
 "testing"
 "github.com/kevinpollet/pocket-deduper/client"
)

func TestReviewEmptyResolvedURLsCollapse(t *testing.T) {
 items:=map[string]client.Item{"a":{ItemID:"a",GivenURL:"https://one.invalid"},"b":{ItemID:"b",GivenURL:"https://two.invalid"}}
 got:=GetDuplicateItems(items)
 if len(got)!=1 {t.Fatalf("got %d duplicates",len(got))}
}
func TestReviewOnlyResolvedURLDeterminesIdentity(t *testing.T) {
 items:=map[string]client.Item{"a":{ItemID:"a",ResolvedURL:"https://same.invalid",Favorite:"1"},"b":{ItemID:"b",ResolvedURL:"https://same.invalid",Favorite:"0"},"c":{ItemID:"c",ResolvedURL:"https://other.invalid"}}
 for i:=0;i<1000;i++ {got:=GetDuplicateItems(items);if len(got)!=1 || (got[0].ItemID!="a" && got[0].ItemID!="b") {t.Fatalf("unexpected duplicates: %+v",got)}}
}
func TestReviewURLStringsAreNotNormalized(t *testing.T) {
 items:=map[string]client.Item{"a":{ResolvedURL:"https://host.invalid/a"},"b":{ResolvedURL:"https://HOST.invalid/a"},"c":{ResolvedURL:"https://host.invalid/a/"}}
 if got:=GetDuplicateItems(items);len(got)!=0 {t.Fatalf("unexpected duplicates: %+v",got)}
}
func TestReviewEmptyAndUniqueInputs(t *testing.T) {
 if got:=GetDuplicateItems(nil);len(got)!=0 {t.Fatal(got)}
 items:=map[string]client.Item{"a":{ResolvedURL:"a"},"b":{ResolvedURL:"b"}}
 if got:=GetDuplicateItems(items);len(got)!=0 {t.Fatal(got)}
}
