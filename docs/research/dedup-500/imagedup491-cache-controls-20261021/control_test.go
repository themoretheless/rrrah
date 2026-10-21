package hash

import (
 "image"
 "image/png"
 "os"
 "path/filepath"
 "testing"
)

func TestReview491DeletedPathStillCached(t *testing.T) {
 dir:=t.TempDir(); path:=filepath.Join(dir,"image.jpg")
 b,err:=os.ReadFile("../testimages/iceland.jpg"); if err!=nil {t.Fatal(err)}
 if err=os.WriteFile(path,b,0600);err!=nil {t.Fatal(err)}
 c,err:=NewCache(filepath.Join(dir,"cache.json"),"review491deleted",1);if err!=nil {t.Fatal(err)}
 first,err:=c.GetHash(path);if err!=nil {t.Fatal(err)}
 if err=os.Remove(path);err!=nil {t.Fatal(err)}
 stale,err:=c.GetHash(path);if err!=nil {t.Fatal(err)}
 if stale!=first {t.Fatal("expected observed stale pointer reuse")}
 t.Logf("Deleted path accepted from cache: dimensions=%dx%d",stale.Width,stale.Height)
 if err=c.Persist();err!=nil {t.Fatal(err)}
}
func TestReview491ReloadLosesDimensions(t *testing.T) {
 dir:=t.TempDir();cachefile:=filepath.Join(dir,"cache.json")
 c,err:=NewCache(cachefile,"review491reload",1);if err!=nil {t.Fatal(err)}
 path:="../testimages/iceland.jpg"
 first,err:=c.GetHash(path);if err!=nil {t.Fatal(err)}
 if first.Width!=640 || first.Height!=426 {t.Fatalf("unexpected fixture dimensions: %dx%d",first.Width,first.Height)}
 if err=c.Persist();err!=nil {t.Fatal(err)}
 restored,err:=NewCache(cachefile,"review491reload",1);if err!=nil {t.Fatal(err)}
 after,err:=restored.GetHash(path);if err!=nil {t.Fatal(err)}
 if after.Width!=0 || after.Height!=0 || after.GetHash()!=first.GetHash() {t.Fatal("expected same hash with lost dimensions")}
 t.Logf("Persisted cache restores dimensions %dx%d from %dx%d",after.Width,after.Height,first.Width,first.Height)
 if err=restored.Persist();err!=nil {t.Fatal(err)}
}
func TestReview491PngDecodeRejected(t *testing.T) {
 dir:=t.TempDir(); path:=filepath.Join(dir,"valid.png")
 f,err:=os.Create(path);if err!=nil {t.Fatal(err)}
 if err=png.Encode(f,image.NewRGBA(image.Rect(0,0,2,2)));err!=nil {t.Fatal(err)}
 if err=f.Close();err!=nil {t.Fatal(err)}
 c,err:=NewCache(filepath.Join(dir,"cache.json"),"review491png",1);if err!=nil {t.Fatal(err)}
 _,err=c.GetHash(path);if err==nil {t.Fatal("expected PNG rejected by JPEG-only decode")}
 t.Log(err)
 if err=c.Persist();err!=nil {t.Fatal(err)}
}
