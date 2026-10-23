using DupeClear.Helpers;
byte[] Join(params byte[][] parts) => parts.SelectMany(x => x).ToArray();
byte[] Segment(byte marker, params byte[] payload) => Join(new byte[]{255,marker,0,(byte)(payload.Length+2)},payload);
byte[] Patch(byte[] bytes) { using var input=new MemoryStream(bytes); using var output=new MemoryStream(); JpegPatcher.PatchAwayExif(input,output); return output.ToArray(); }
void Check(bool condition,string label) { if(!condition) throw new Exception(label); Console.WriteLine("PASS "+label); }
byte[] soi={255,216}, tail={255,217};
var a=Join(soi,Segment(226,1,2,3),tail);
var b=Join(soi,Segment(226,9,8,7),tail);
Check(!a.SequenceEqual(b) && Patch(a).SequenceEqual(Patch(b)),"different leading APP2 payloads collapse");
Check(Patch(a).SequenceEqual(Join(soi,tail)),"leading APP2 removed completely");
var comment=Segment(254,42);
var lateA=Join(soi,comment,Segment(226,1,2,3),tail);
var lateB=Join(soi,comment,Segment(226,9,8,7),tail);
Check(!Patch(lateA).SequenceEqual(Patch(lateB)),"APP2 after COM is retained");
Check(Patch(Join(soi,Segment(238,4,5),tail)).SequenceEqual(Join(soi,tail)),"leading APP14 also removed");
Check(Patch(Join(soi,tail)).SequenceEqual(Join(soi,tail)),"no-APP positive control preserved");
Console.WriteLine("5 checks passed; synthetic marker streams, not decoded JPEG images.");
