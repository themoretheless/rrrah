import co.nubetech.hiho.mapreduce.lib.input.*;
import org.apache.hadoop.conf.Configuration;
import org.apache.hadoop.fs.Path;
import org.apache.hadoop.mapreduce.*;
import org.apache.hadoop.mapreduce.lib.input.FileSplit;
import java.nio.file.Files;
import java.util.Arrays;
public class StreamBoundaryControl{
 static String read(java.nio.file.Path source,byte[] expected)throws Exception{
  TaskAttemptContext context=new TaskAttemptContext(new Configuration(),new TaskAttemptID());
  FileSplit split=new FileSplit(new Path(source.toUri().toString()),0,expected.length,new String[0]);
  FileStreamRecordReader reader=new FileStreamRecordReader(split,context);reader.initialize(split,context);
  try{if(!reader.nextKeyValue())throw new AssertionError("missing record");String key=reader.getCurrentKey().toString();if(!Arrays.equals(expected,reader.getCurrentValue().readAllBytes())||reader.nextKeyValue())throw new AssertionError("payload/EOF");System.out.println("READ key="+key+" bytes="+expected.length+" progressAfterEOF="+reader.getProgress());return key;}finally{reader.close();}
 }
 public static void main(String[] args)throws Exception{
  java.nio.file.Path base=java.nio.file.Path.of(args[0]);java.nio.file.Path a=base.resolve("left/same.bin"),b=base.resolve("right/same.bin"),empty=base.resolve("empty.bin");Files.createDirectories(a.getParent());Files.createDirectories(b.getParent());byte[] first={1,2,3},second={4,5,6};Files.write(a,first);Files.write(b,second);Files.write(empty,new byte[0]);
  String left=read(a,first),right=read(b,second);if(!left.equals(right))throw new AssertionError("expected basename-only identity observation");System.out.println("OBSERVED distinct full paths/different payloads emit same basename key");read(empty,new byte[0]);System.out.println("stream checks=4;3complete payload+EOF checks,1basename identity observation");
 }
}
