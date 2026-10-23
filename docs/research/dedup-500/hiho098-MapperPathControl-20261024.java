import co.nubetech.hiho.common.HIHOConf;
import co.nubetech.hiho.merge.*;
import org.apache.hadoop.conf.Configuration;
import org.apache.hadoop.fs.Path;
import org.apache.hadoop.io.Text;
import org.apache.hadoop.mapreduce.*;
import org.apache.hadoop.mapreduce.lib.input.FileSplit;
import java.util.*;
public class MapperPathControl {
 static class Reader extends RecordReader<Text,Text>{boolean used;public void initialize(InputSplit s,TaskAttemptContext c){}public boolean nextKeyValue(){if(used)return false;used=true;return true;}public Text getCurrentKey(){return new Text("key");}public Text getCurrentValue(){return new Text("payload");}public float getProgress(){return used?1:0;}public void close(){}}
 static class Writer extends RecordWriter<Object,Object>{List<Boolean> old=new ArrayList<>();public void write(Object k,Object v){old.add(((HihoValue)v).getIsOld());}public void close(TaskAttemptContext c){}}
 static class Reporter extends StatusReporter{Counters c=new Counters();public Counter getCounter(Enum<?> e){return c.findCounter(e);}public Counter getCounter(String g,String n){return c.findCounter(g,n);}public void progress(){}public void setStatus(String s){}}
 static boolean run(boolean mode,String old,String fresh,String input)throws Exception{
  Mapper mapper=mode?new MergeValueMapper<>():new MergeKeyMapper<>();Configuration conf=new Configuration();conf.set(HIHOConf.MERGE_OLD_PATH,old);conf.set(HIHOConf.MERGE_NEW_PATH,fresh);Writer out=new Writer();
  Mapper.Context context=mapper.new Context(conf,new TaskAttemptID(),new Reader(),out,null,new Reporter(),new FileSplit(new Path(input),0,1,new String[0]));mapper.run(context);if(out.old.size()!=1)throw new AssertionError(out.old.toString());return out.old.get(0);
 }
 public static void main(String[] args)throws Exception{
  int checks=0;
  for(boolean mode:new boolean[]{false,true}){
   if(!run(mode,"/owned/old","/owned/new","/owned/old/item"))throw new AssertionError("old positive");checks++;
   if(run(mode,"/owned/old","/owned/new","/owned/new/item"))throw new AssertionError("new positive");checks++;
   if(!run(mode,"/owned/data","/owned/data-new","/owned/data-new/item"))throw new AssertionError("prefix observation");checks++;
   if(!run(mode,"/owned/data","/owned/fresh","/owned/data-other/item"))throw new AssertionError("sibling observation");checks++;
   System.out.println("PASS old/new positive controls; OBSERVED new prefix and unrelated sibling labelled old; valueMode="+mode);
  }
  System.out.println("mapper path checks="+checks+";realMapper.Context;no filesystem reads");
 }
}
