import co.nubetech.hiho.dedup.DedupJob;
import co.nubetech.hiho.merge.MergeJob;
import org.apache.hadoop.conf.Configuration;
import org.apache.hadoop.io.Text;
import org.apache.hadoop.mapreduce.*;
import java.util.*;
public class JobFailureControl {
 static int failures;
 public static class FailingFormat extends OutputFormat<Text,Text>{
  public FailingFormat(){failures++;throw new IllegalStateException("owned output-format construction failure before job submission");}
  public RecordWriter<Text,Text> getRecordWriter(TaskAttemptContext c){throw new AssertionError("unreachable");}
  public void checkOutputSpecs(JobContext c){throw new AssertionError("unreachable");}
  public OutputCommitter getOutputCommitter(TaskAttemptContext c){throw new AssertionError("unreachable");}
 }
 public static void main(String[] args)throws Exception{
  List<String> common=new ArrayList<>(List.of("-inputFormat","org.apache.hadoop.mapreduce.lib.input.SequenceFileInputFormat","-inputKeyClassName","org.apache.hadoop.io.Text","-inputValueClassName","org.apache.hadoop.io.Text","-outputPath","/owned/no-output-created","-outputFormat",FailingFormat.class.getName()));
  List<String> dedup=new ArrayList<>(common);dedup.addAll(List.of("-inputPath","/owned/no-input-read","-dedupBy","key"));DedupJob d=new DedupJob();d.setConf(new Configuration());int first=d.run(dedup.toArray(new String[0]));
  if(first!=0||failures!=1)throw new AssertionError("dedup return="+first+" failures="+failures);
  System.out.println("OBSERVED DedupJob returns0 after output-format construction failure");
  List<String> merge=new ArrayList<>(common);merge.addAll(List.of("-oldPath","/owned/no-old-read","-newPath","/owned/no-new-read","-mergeBy","key"));MergeJob m=new MergeJob();m.setConf(new Configuration());int second=m.run(merge.toArray(new String[0]));
  if(second!=0||failures!=2)throw new AssertionError("merge return="+second+" failures="+failures);
  System.out.println("OBSERVED MergeJob returns0 after output-format construction failure");System.out.println("checks=2; no job submitted; no file input/output");
 }
}
