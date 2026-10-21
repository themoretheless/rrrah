using System;
using System.Linq;
using NutzCode.Libraries.PerceptualImage;
using NutzCode.Libraries.PerceptualImage.Hash;
class Program{
 static void Check(bool ok,string message){if(!ok)throw new Exception(message);Console.WriteLine("PASS "+message);}
 static void Main(){
 var zero=new Hash256Item<int>(0,new byte[32]);
 for(int bit=0;bit<256;bit++){var bytes=new byte[32];bytes[bit/8]=(byte)(1<<(bit%8));var item=new Hash256Item<int>(bit,bytes);CheckQuiet(item.ByteArray.SequenceEqual(bytes)&&zero.CalculateDistance(item)==1);}
 Check(true,"all256 scalar one-hot hash distances and byte roundtrips");
 var full=Enumerable.Repeat((byte)255,32).ToArray();Check(zero.CalculateDistance(new Hash256Item<int>(1,full))==256,"scalar full256bit distance");
 float[] a={0,1,2,3,4,5,6,7,100};float[] b={0,1,2,3,4,5,6,7,101};Check(PerceptualImageHash.GenerateHashes(a).Length==1&&PerceptualImageHash.GenerateHashes(a).SequenceEqual(PerceptualImageHash.GenerateHashes(b)),"nine coefficient inputs silently truncate final bit");
 bool failed=false;try{PerceptualImageHash.GenerateHashes(Array.Empty<float>());}catch(IndexOutOfRangeException){failed=true;}Check(failed,"empty coefficient array throws without validation");
 var image=new int[32,32];for(int y=0;y<32;y++)for(int x=0;x<32;x++)image[x,y]=(x*7+y*11)%256;
 var hash=PerceptualImageHash.Hash(image,32,32);Check(hash.Length==32&&hash.SequenceEqual(PerceptualImageHash.Hash(image,32,32)),"original integer image pipeline returns deterministic32bytes for synthetic fixture");
 Console.WriteLine("RESULT 5 controls passed; original C# sources; no encoded image decoding or corpus qualification");
 }
 static void CheckQuiet(bool b){if(!b)throw new Exception("onehot failure");}
}
