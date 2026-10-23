import co.nubetech.hiho.dedup.HihoTuple;
import co.nubetech.hiho.merge.*;
import org.apache.hadoop.conf.Configuration;
import org.apache.hadoop.io.*;
import org.apache.hadoop.mapred.RawKeyValueIterator;
import org.apache.hadoop.mapreduce.*;
import org.apache.hadoop.util.Progress;
import java.io.*;
import java.util.*;

public class MergeContextControl {
 static byte[] encode(Writable value)throws Exception{ByteArrayOutputStream b=new ByteArrayOutputStream();value.write(new DataOutputStream(b));return b.toByteArray();}
 static class Raw implements RawKeyValueIterator {
  byte[] key;List<byte[]> values=new ArrayList<>();int index=-1;DataInputBuffer k=new DataInputBuffer(),v=new DataInputBuffer();
  Raw(String[] texts,boolean[] old)throws Exception{
   HihoTuple<Text> t=new HihoTuple<>();t.setKey(new Text("group"));key=encode(t);
   for(int i=0;i<texts.length;i++){HihoValue<Text> h=new HihoValue<>();h.setVal(new Text(texts[i]));h.setIsOld(old[i]);values.add(encode(h));}
  }
  public boolean next(){index++;if(index>=values.size())return false;k.reset(key,key.length);byte[] b=values.get(index);v.reset(b,b.length);return true;}
  public DataInputBuffer getKey(){return k;}public DataInputBuffer getValue(){return v;}public void close(){}public Progress getProgress(){return new Progress();}
 }
 static class Reporter extends StatusReporter {
  Counters counters=new Counters();public Counter getCounter(Enum<?> e){return counters.findCounter(e);}public Counter getCounter(String g,String n){return counters.findCounter(g,n);}public void progress(){}public void setStatus(String s){}
 }
 static class Writer extends RecordWriter<Object,Object>{List<String> rows=new ArrayList<>();public void write(Object k,Object v){rows.add(k+"="+v);}public void close(TaskAttemptContext c){}}
 static String run(boolean valueMode,String[] texts,boolean[] old)throws Exception{
  Reducer reducer=valueMode?new MergeValueReducer<>():new MergeKeyReducer<>();Raw raw=new Raw(texts,old);Writer writer=new Writer();Reporter reporter=new Reporter();Configuration conf=new Configuration();
  Reducer.Context context=reducer.new Context(conf,new TaskAttemptID(),raw,reporter.getCounter("owned","groups"),reporter.getCounter("owned","values"),writer,null,reporter,WritableComparator.get(HihoTuple.class),HihoTuple.class,HihoValue.class);
  reducer.run(context);if(writer.rows.size()!=1)throw new AssertionError(writer.rows.toString());return writer.rows.get(0);
 }
 public static void main(String[] args)throws Exception{
  int checks=0;
  for(boolean mode:new boolean[]{false,true}){
   for(boolean newFirst:new boolean[]{false,true}){
    String[] texts=newFirst?new String[]{"new","old"}:new String[]{"old","new"};boolean[] old=newFirst?new boolean[]{false,true}:new boolean[]{true,false};
    String result=run(mode,texts,old),expected=mode?"new=group":"group=new";if(!result.equals(expected))throw new AssertionError(result);checks++;System.out.println("PASS single-new survives both orders, valueMode="+mode+" newFirst="+newFirst+" result="+result);
   }
   String forward=run(mode,new String[]{"newA","newB"},new boolean[]{false,false});String reverse=run(mode,new String[]{"newB","newA"},new boolean[]{false,false});
   if(forward.equals(reverse))throw new AssertionError("Expected last-new order dependence");checks++;System.out.println("OBSERVED two-new order dependence, valueMode="+mode+" forward="+forward+" reverse="+reverse);
  }
  System.out.println("context checks="+checks+";8real reducer.run invocations;no cluster");
 }
}
