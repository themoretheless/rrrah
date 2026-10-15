using System;
using System.IO;
using System.Linq;
using System.Collections;
using System.Collections.Generic;
using System.Security.Cryptography;
using Xnlab.SharpDups.Infrastructure;
using Xnlab.SharpDups.Logic;

class Review {
 static void Check(bool value,string message) {if(!value)throw new Exception(message);}
 static int count;
 static void Run(string name,Action test){test();count++;Console.WriteLine("PASS "+name);}
 sealed class Once : IEnumerable<string> {
  string[] files;bool consumed;
  public Once(params string[] values){files=values;}
  public IEnumerator<string> GetEnumerator(){if(consumed)return Enumerable.Empty<string>().GetEnumerator();consumed=true;return ((IEnumerable<string>)files).GetEnumerator();}
  IEnumerator IEnumerable.GetEnumerator()=>GetEnumerator();
 }
 static void Main() {
  string root=Path.Combine(Path.GetTempPath(),"rrrah-sharp77-"+Guid.NewGuid());Directory.CreateDirectory(root);
  string a=Path.Combine(root,"a"),b=Path.Combine(root,"b");
  try {
   Run("full_md5_seven_stream_boundaries",()=>{
    foreach(int size in new[]{0,1,1023,1024,1025,4097,65537}){byte[] data=Enumerable.Range(0,size).Select(i=>(byte)(i*17+3)).ToArray();File.WriteAllBytes(a,data);Check(HashTool.HashFileBytes(a,1024).SequenceEqual(MD5.HashData(data)),"full MD5 oracle");}
   });
   Run("partial_section_pads_unread_tail_and_overstates_read_size",()=>{
    File.WriteAllBytes(a,Enumerable.Range(0,11).Select(i=>(byte)i).ToArray());int read;
    var hash=HashTool.HashFileBytes(a,8,4,1024,out read);Check(read==4,"reported requested count");Check(hash.SequenceEqual(MD5.HashData(new byte[]{8,9,10,0})),"three actual bytes plus zero padding");
    hash=HashTool.HashFileBytes(a,20,4,1024,out read);Check(read==4&&hash.SequenceEqual(MD5.HashData(new byte[4])),"beyond EOF reports4 and hashes zeros");
   });
   Run("one_and_two_byte_unequal_files_published_as_duplicates",()=>{
    foreach(int size in new[]{1,2}){File.WriteAllBytes(a,Enumerable.Repeat((byte)1,size).ToArray());File.WriteAllBytes(b,Enumerable.Repeat((byte)2,size).ToArray());var result=new ProgressiveDupDetector().Find(new[]{a,b},1,3,4);Check(result.Duplicates.Count==1&&result.Duplicates[0].Items.Count()==2,"tiny false equality");Check(!File.ReadAllBytes(a).SequenceEqual(File.ReadAllBytes(b)),"actual unequal bytes");}
   });
   Run("progressive_pair_comparison_omits_unsampled_remainder",()=>{
    var data=new byte[11];File.WriteAllBytes(a,data);data[8]=1;File.WriteAllBytes(b,data);
    var source=ProgressiveDupDetector.ProgressiveHashFile(a,3,4);Check(source.HashSections.Count==2,"floor two blocks");Check(ProgressiveDupDetector.ProgressiveCompareFile(source,b,3,4),"actual false equality");Check(HashTool.HashFile(a,1024)!=HashTool.HashFile(b,1024),"independent full hash differs");
   });
   Run("progressive_batch_publishes_same_tail_false_equality",()=>{
    var result=new ProgressiveDupDetector().Find(new[]{a,b},1,3,4);Check(result.Duplicates.Count==1,"unsampled remainder false group");
   });
   Run("first_complete_block_difference_is_rejected",()=>{
    var data=new byte[11];File.WriteAllBytes(a,data);data[1]=1;File.WriteAllBytes(b,data);var source=ProgressiveDupDetector.ProgressiveHashFile(a,3,4);Check(!ProgressiveDupDetector.ProgressiveCompareFile(source,b,3,4),"covered difference");
   });
   Run("single_use_enumeration_consumed_by_total_count",()=>{
    File.WriteAllBytes(a,new byte[12]);File.WriteAllBytes(b,new byte[12]);var result=new ProgressiveDupDetector().Find(new Once(a,b),1,3,4);Check(result.TotalFiles==2&&result.TotalComparedFiles==0&&result.Duplicates.Count==0,"count consumed input");
   });
   Run("missing_file_error_contract_differs_between_versions",()=>{
    var missing=Path.Combine(root,"missing");bool threw=false;try{new DupDetector().Find(new[]{missing},1);}catch(FileNotFoundException){threw=true;}Check(threw,"V1 throws before processing catch");var result=new ProgressiveDupDetector().Find(new[]{missing},1,3,4);Check(result.FailedToProcessFiles.SequenceEqual(new[]{missing}),"V3 records missing source");
   });
   Console.WriteLine("TOTAL "+count+" PASS");
  }finally{Directory.Delete(root,true);}
 }
}
