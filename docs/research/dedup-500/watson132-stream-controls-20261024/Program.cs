using WatsonDedupe;
using WatsonDedupe.Database;
using System.Text.Json;
void Check(bool ok,string name){if(!ok)throw new Exception(name);}
DedupeStream Create(byte[] bytes)=>new(new DedupeObject{Key="owned",OriginalLength=4},new DbProvider(),new DedupeCallbacks{ReadChunk=_=>bytes});
var normal=Create(new byte[]{1,2,3,4});var b=new byte[16];Check(normal.Read(b,0,4)==4&&b.Take(4).SequenceEqual(new byte[]{1,2,3,4}),"normal read");
var offset=Create(new byte[]{1,2,3,4});Check(offset.Read(new byte[16],8,4)==0&&offset.Position==0,"buffer offset mistaken for stream position");
var seek=Create(new byte[]{1,2,3,4});seek.Position=4;try{seek.Seek(4,SeekOrigin.Begin);throw new Exception("expected seek refusal");}catch(ArgumentOutOfRangeException){}
var corrupt=Create(new byte[]{9,9,9,9});var output=new byte[4];Check(corrupt.Read(output,0,4)==4&&output.All(x=>x==9),"callback corruption accepted");
Console.WriteLine(JsonSerializer.Serialize(new{controls_passed=4,scope="Original full DedupeStream class;native .NET10;explicit metadata/database/callback DTO seams;no real DB,persistence or full library",findings=new[]{"Read destination offset8 on length4 stream returns0 at position0","Position=end accepted but Seek(end,Begin) rejected","Wrong callback bytes returned without hash verification","Healthy buffer read positive"}}));
namespace WatsonDedupe{public class DedupeObject{public string Key;public long OriginalLength;}public class DedupeObjectMap{public long ChunkAddress;public int ChunkLength=4;public string ChunkKey="owned-original-hash";}public class DedupeCallbacks{public Func<string,byte[]> ReadChunk;}}
namespace WatsonDedupe.Database{public class DbProvider{public DedupeObjectMap GetObjectMapForPosition(string key,long position)=>position<4?new DedupeObjectMap():null;}}
