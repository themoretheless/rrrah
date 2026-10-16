using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Reflection;
using System.Text.Json;
using DupImageLib;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;
class Program {
 static readonly List<object> rows=new();
 static void Check(string name,Func<bool> fn) {bool okay=fn();if(!okay)throw new Exception(name);rows.Add(new{name,passed=okay});}
 static MemoryStream Solid(byte value) {using var im=new Image<Rgba32>(32,32,new Rgba32(value,value,value,255));var s=new MemoryStream();im.SaveAsPng(s);s.Position=0;return s;}
 static int Bits(ulong x){int n=0;while(x!=0){n+=(int)(x&1);x>>=1;}return n;}
 static void Main() {
  var original=new List<object>();
  foreach(var type in new[]{typeof(DupImageLib.Tests.ImageHashesTests),typeof(DupImageLib.Tests.ImageSharpTransformerTests)})
   foreach(var method in type.GetMethods().OrderBy(m=>m.Name)) {
    var attrs=method.GetCustomAttributes().ToArray();
    bool fact=attrs.Any(a=>a.GetType().FullName=="Xunit.FactAttribute");bool theory=attrs.Any(a=>a.GetType().FullName=="Xunit.TheoryAttribute");
    if(!fact&&!theory)continue;
    var data=new List<object[]>();if(fact)data.Add(Array.Empty<object>());
    if(theory) foreach(var a in attrs.Where(a=>a.GetType().FullName=="Xunit.InlineDataAttribute"))
      foreach(var c in (IEnumerable<object[]>)a.GetType().GetMethod("GetData").Invoke(a,new object[]{method}))data.Add(c);
    foreach(var c in data) {var parameters=method.GetParameters();var converted=c.Select((v,i)=>Convert.ChangeType(v,parameters[i].ParameterType)).ToArray();string error=null;try{method.Invoke(Activator.CreateInstance(type),converted);}catch(TargetInvocationException e){error=e.InnerException.ToString();}original.Add(new{name=type.Name+"."+method.Name,args=converted.Select(x=>x.ToString()).ToArray(),passed=error==null,error});}
   }
  var h=new ImageHashes();
  Check("empty_hash_arrays_return_NaN",()=>float.IsNaN(ImageHashes.CompareHashes(Array.Empty<ulong>(),Array.Empty<ulong>())));
  Check("null_hash_array_throws",()=>{try{ImageHashes.CompareHashes((ulong[])null,new ulong[1]);return false;}catch(NullReferenceException){return true;}});
  Check("mismatched_hash_arrays_throw",()=>{try{ImageHashes.CompareHashes(new ulong[1],new ulong[2]);return false;}catch(ArgumentException){return true;}});
  Check("all_64_single_bit_distances",()=>Enumerable.Range(0,64).All(i=>ImageHashes.CompareHashes(0,1UL<<i)==63/64f));
  Check("1000_scalar_popcount_oracle",()=>{ulong x=17;for(int i=0;i<1000;i++){x=unchecked(x*6364136223846793005UL+1);ulong y=~(x>>7);if(ImageHashes.CompareHashes(x,y)!=1-Bits(x^y)/64f)return false;}return true;});
  Check("1000_four_word_popcount_oracle",()=>{ulong x=91;for(int i=0;i<1000;i++){var a=new ulong[4];var b=new ulong[4];int bits=0;for(int j=0;j<4;j++){x=unchecked(x*6364136223846793005UL+1);a[j]=x;b[j]=x>>9;bits+=Bits(a[j]^b[j]);}if(ImageHashes.CompareHashes(a,b)!=1-bits/256f)return false;}return true;});
  Check("real_black_white_average_collision",()=>{using var a=Solid(0);using var b=Solid(255);return h.CalculateAverageHash64(a)==0&&h.CalculateAverageHash64(b)==0;});
  Check("real_black_white_median64_collision",()=>{using var a=Solid(0);using var b=Solid(255);return h.CalculateMedianHash64(a)==0&&h.CalculateMedianHash64(b)==0;});
  Check("real_black_white_median256_collision",()=>{using var a=Solid(0);using var b=Solid(255);return h.CalculateMedianHash256(a).All(x=>x==0)&&h.CalculateMedianHash256(b).All(x=>x==0);});
  Check("real_black_white_difference64_collision",()=>{using var a=Solid(0);using var b=Solid(255);return h.CalculateDifferenceHash64(a)==0&&h.CalculateDifferenceHash64(b)==0;});
  Check("real_black_white_difference256_collision",()=>{using var a=Solid(0);using var b=Solid(255);return h.CalculateDifferenceHash256(a).All(x=>x==0)&&h.CalculateDifferenceHash256(b).All(x=>x==0);});
  Check("real_black_dct_zero",()=>{using var a=Solid(0);return h.CalculateDctHash(a)==0;});
  Check("real_invalid_image_throws",()=>{using var s=new MemoryStream(new byte[]{1,2,3});try{h.CalculateMedianHash64(s);return false;}catch(UnknownImageFormatException){return true;}});
  Check("caller_stream_remains_open",()=>{using var s=Solid(128);h.CalculateMedianHash64(s);return s.CanRead;});
  Check("short_transformer_buffer_throws",()=>{var fake=new ImageHashes(new ShortTransformer());try{fake.CalculateAverageHash64((Stream)null);return false;}catch(IndexOutOfRangeException){return true;}});
  Check("null_transformer_throws_on_use",()=>{var fake=new ImageHashes(null);try{fake.CalculateAverageHash64((Stream)null);return false;}catch(NullReferenceException){return true;}});
  Console.WriteLine(JsonSerializer.Serialize(new{runtime=System.Runtime.InteropServices.RuntimeInformation.FrameworkDescription,architecture=System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture.ToString(),original,controls=rows}));
 }
 class ShortTransformer:IImageTransformer {public byte[] TransformImage(Stream s,int w,int h)=>new byte[1];}
}
