package main
import("testing";"os";"io";"encoding/json")
func group(t *testing.T,body string)JSONOutput{t.Helper();f,err:=os.CreateTemp(t.TempDir(),"list");if err!=nil{t.Fatal(err)};if _,err=f.WriteString(body);err!=nil{t.Fatal(err)};f.Close();old:=os.Stdout;r,w,err:=os.Pipe();if err!=nil{t.Fatal(err)};os.Stdout=w;defer func(){os.Stdout=old}();err=deduplicate(f.Name(),true);w.Close();data,readErr:=io.ReadAll(r);r.Close();if err!=nil||readErr!=nil{t.Fatal(err,readErr)};var out JSONOutput;if err=json.Unmarshal(data,&out);err!=nil{t.Fatal(err,string(data))};return out}
func TestFinalUnterminatedRecordIsLost(t *testing.T){out:=group(t,"hash first\nhash second");if out.Count!=0{t.Fatal(out)}}
func TestNewlineTerminatedPairIsGrouped(t *testing.T){out:=group(t,"hash first\nhash second\n");if out.Count!=1||len(out.Duplicates[0])!=2{t.Fatal(out)}}
func TestSamePathRepeatedCountsAsDuplicate(t *testing.T){out:=group(t,"not-a-digest same\nnot-a-digest same\n");if out.Count!=1||out.Duplicates[0][0]!=out.Duplicates[0][1]{t.Fatal(out)}}
func TestEmptyDigestGroupsDifferentPaths(t *testing.T){out:=group(t," first\n second\n");if out.Count!=1||len(out.Duplicates[0])!=2{t.Fatal(out)}}
