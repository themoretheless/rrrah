package co.nubetech.hiho.mapreduce.lib.output;
import co.nubetech.hiho.common.HIHOConf;
import org.apache.hadoop.conf.Configuration;
import org.apache.hadoop.io.Text;
import org.apache.hadoop.mapreduce.*;
import org.apache.hadoop.mapreduce.lib.output.FileOutputFormat;
import java.io.*;
import java.nio.file.Files;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
public class OutputBoundaryControl {
 static int checks;static void require(boolean b,String s){if(!b)throw new AssertionError(s);checks++;System.out.println("PASS "+s);}
 static class Closed extends ByteArrayOutputStream{boolean closed;public void close(){closed=true;}}
 public static void main(String[] args)throws Exception{
  Closed bytes=new Closed();NoKeyOnlyValueOutputFormat.NoKeyRecordWriter<Object,Object> writer=new NoKeyOnlyValueOutputFormat.NoKeyRecordWriter<>(new DataOutputStream(bytes));Text reused=new Text("long stale backing contents");reused.set("abc".getBytes(StandardCharsets.UTF_8),0,3);writer.write("ignored",reused);require(Arrays.equals(bytes.toByteArray(),"abc\n".getBytes(StandardCharsets.UTF_8)),"Text logical length excludes stale backing bytes");
  bytes.reset();writer.write("ignored",new Text("я\u0000é"));require(Arrays.equals(bytes.toByteArray(),"я\u0000é\n".getBytes(StandardCharsets.UTF_8)),"UTF8 and NUL content preserved plus documented newline");
  NoKeyOnlyValueOutputFormat.NoKeyRecordWriter<Object,Object> failing=new NoKeyOnlyValueOutputFormat.NoKeyRecordWriter<>(new DataOutputStream(new OutputStream(){public void write(int b)throws IOException{throw new IOException("owned write failure");}}));try{failing.write(null,new Text("x"));throw new AssertionError();}catch(IOException expected){checks++;System.out.println("PASS output IOException propagated");}
  writer.close(null);require(bytes.closed,"close reaches underlying output");try{writer.write(null,null);throw new AssertionError();}catch(NullPointerException expected){checks++;System.out.println("OBSERVED null value rejected with NullPointerException");}
  TaskAttemptContext missing=new TaskAttemptContext(new Configuration(),new TaskAttemptID());try{new AppendTextOutputFormat().getDefaultWorkFile(missing,"");throw new AssertionError();}catch(NullPointerException expected){checks++;System.out.println("OBSERVED missing append flag gives NullPointerException");}
  for(boolean sequence:new boolean[]{false,true}){
   java.nio.file.Path directory=java.nio.file.Path.of(args[0]).resolve(sequence?"sequence":"text");Files.createDirectories(directory);for(String name:new String[]{"part--00000","part--00002","_SUCCESS"})Files.write(directory.resolve(name),new byte[0]);
   Configuration conf=new Configuration();conf.set(HIHOConf.IS_APPEND,"true");Job job=new Job(conf);FileOutputFormat.setOutputPath(job,new org.apache.hadoop.fs.Path(directory.toUri().toString()));TaskAttemptContext context=new TaskAttemptContext(job.getConfiguration(),new TaskAttemptID());
   org.apache.hadoop.fs.Path candidate=sequence?new AppendSequenceFileOutputFormat<>().getDefaultWorkFile(context,""):new AppendTextOutputFormat().getDefaultWorkFile(context,"");require(candidate.getName().equals("part--00002")&&Files.exists(directory.resolve(candidate.getName())),"sparse existing filenames cause append destination-name collision;sequence="+sequence);System.out.println("candidate work path="+candidate);
  }
  System.out.println("output checks="+checks+";no writer commit/cluster/FTP");
 }
}
