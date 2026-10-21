using System;
using NutzCode.Libraries.PerceptualImage.Hash;
using NutzCode.Libraries.PerceptualImage.Tree.VantagePoint;
class Program {
 static void Check(bool b,string message){if(!b)throw new Exception(message);Console.WriteLine("PASS "+message);}
 static void Main(){
 var zero=new Hash256Item<int>(0,0,0,0,0);var one=new Hash256Item<int>(1,1,0,0,0);
 var singleton=new VPTree<Hash256Item<int>>(new HashList<Hash256Item<int>>{zero});
 Check(singleton.Range(zero,257).Count==0,"singleton population loses its only item");
 var pair=new VPTree<Hash256Item<int>>(new HashList<Hash256Item<int>>{zero,one});
 Check(pair.Range(zero,257).Count==1,"two-item population returns only one item at full radius");
 bool failed=false;try{new VPTree<Hash256Item<int>>(new HashList<Hash256Item<int>>());}catch(ArgumentOutOfRangeException){failed=true;}
 Check(failed,"empty population indexes nonexistent item");
 var bytes=new byte[32];var full=new byte[32];Array.Fill(full,(byte)255);
 Check(new Hash256LookUpItem<int>(0,bytes).CalculateDistance(new Hash256LookUpItem<int>(1,full))==0,"lookup distance reports zero for all256 opposite bits");
 failed=false;try{new DynamicVPTree<Hash256Item<int>,int>().Add(one);}catch(NullReferenceException){failed=true;}
 Check(failed,"default dynamic tree Add encounters uninitialized lookup");
 Console.WriteLine("RESULT 5 original C# controls passed; no image hashing pipeline");
 }
}
