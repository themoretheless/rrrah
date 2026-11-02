using WatsonDedupe;
using System.Text.Json;
void Check(bool ok,string name){if(!ok)throw new Exception(name);}
var partial=new PartialStream(new byte[]{1,2,3,4});var actual=DedupeCommon.ReadBytesFromStream(partial,4,out long count);Check(count==4&&actual.SequenceEqual(new byte[]{3,4,0,0}),"partial overwrite");
var exhausted=new PartialStream(new byte[]{1,2});try{DedupeCommon.ReadBytesFromStream(exhausted,4,out _);throw new Exception("no EOF guard");}catch(EndOfStreamException){Check(exhausted.ZeroReads==3,"repeated zero reads");}
try{DedupeCommon.IsZeroBytes(new byte[]{0},2);throw new Exception("missing boundary exception");}catch(IndexOutOfRangeException){}
var full=DedupeCommon.ReadBytesFromStream(new MemoryStream(new byte[]{1,2,3,4}),4,out long fullCount);Check(fullCount==4&&full.SequenceEqual(new byte[]{1,2,3,4}),"full positive");
Check(Convert.ToHexString(DedupeCommon.Sha256(new byte[]{97,98,99})).ToLowerInvariant()=="ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad","sha256 known vector");
Console.WriteLine(JsonSerializer.Serialize(new{controls_passed=5,partial_result=actual,zero_reads=exhausted.ZeroReads,scope="Unchanged original DedupeCommon;native .NET10;owned MemoryStream and short-read adapter;EOF adapter throws after3 zero returns to bound execution;no full chunk store/database"}));
class PartialStream:MemoryStream{
 public int ZeroReads=0;public PartialStream(byte[] b):base(b){}
 public override int Read(byte[] buffer,int offset,int count){if(Position>=Length){ZeroReads++;if(ZeroReads>=3)throw new EndOfStreamException("owned bounded EOF guard");return 0;}return base.Read(buffer,offset,Math.Min(count,2));}
}
