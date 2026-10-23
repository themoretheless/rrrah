import org.apache.hadoop.conf.Configuration;
import org.apache.hadoop.fs.FileSystem;
import org.apache.hadoop.fs.Path;
import org.apache.hadoop.io.*;
import org.apache.hadoop.util.ReflectionUtils;
public class SequenceFixtureControl {
 public static void main(String[] args)throws Exception{
  Configuration conf=new Configuration();FileSystem fs=FileSystem.getLocal(conf);
  for(String file:args){
   SequenceFile.Reader reader=new SequenceFile.Reader(fs,new Path(file),conf);
   try{
    System.out.println("FILE "+file+" key="+reader.getKeyClassName()+" value="+reader.getValueClassName()+" compressed="+reader.isCompressed()+" block="+reader.isBlockCompressed()+" metadata="+reader.getMetadata());
    Writable key=(Writable)ReflectionUtils.newInstance(reader.getKeyClass(),conf),value=(Writable)ReflectionUtils.newInstance(reader.getValueClass(),conf);int count=0;
    while(reader.next(key,value)){System.out.println("ROW "+key+"="+value);count++;}
    long size=new java.io.File(file).length();if(reader.getPosition()!=size||count!=2)throw new AssertionError("position="+reader.getPosition()+" size="+size+" count="+count);System.out.println("EOF "+reader.getPosition()+" rows="+count);
   }finally{reader.close();}
  }
 }
}
