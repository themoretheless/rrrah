package service
import("bytes";"crypto/sha256";"encoding/hex";"hash/crc32";"os";"path/filepath";"testing";set "github.com/deckarep/golang-set/v2";"github.com/m-manu/go-find-duplicates/fmte")
func TestReviewFullSmallCRC32CollisionPublishesUnequalFiles(t *testing.T){
 fmte.Off();a,_:=hex.DecodeString("7dfebcc06d5e3d22");b,_:=hex.DecodeString("ca587acde9057c9c")
 if bytes.Equal(a,b)||crc32.ChecksumIEEE(a)!=crc32.ChecksumIEEE(b)||sha256.Sum256(a)==sha256.Sum256(b){t.Fatal("invalid independent fixture")}
 dir:=t.TempDir();for name,data:=range map[string][]byte{"a.bin":a,"b.bin":b}{if e:=os.WriteFile(filepath.Join(dir,name),data,0600);e!=nil{t.Fatal(e)}}
 for _,thorough:=range []bool{false,true}{groups,count,savings,files,e:=FindDuplicates([]string{dir},set.NewSet[string](),0,1,thorough);if e!=nil||len(files)!=2||groups==nil{t.Fatalf("scan %v",e)};n:=1;if thorough{n=0};if groups.Size()!=n||count!=int64(n)||savings!=int64(n*8){t.Fatalf("thorough=%v groups=%v count=%v savings=%v",thorough,groups.Size(),count,savings)}}
}
