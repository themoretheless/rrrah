from pathlib import Path
import subprocess,json,shutil
root=Path.cwd();runtime=root/'.dedup-work/hiho098-runtime-20261024';j=next(runtime.glob('*/Contents/Home'));base=root/'.dedup-work/hiho098-split-runtime-20261024';src=base/'src';classes=base/'classes';src.mkdir(parents=True,exist_ok=True);classes.mkdir(exist_ok=True);f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';package='co.nubetech.hiho.mapreduce.lib.db.apache'
for row in json.loads((root/'.dedup-work/hiho098-split-files-20261024.json').read_text()):shutil.copyfile(f/row['local'],src/row['local'].split('-',1)[1])
(src/'MRJobConfig.java').write_text('package '+package+'; public interface MRJobConfig {String NUM_MAPS="mapreduce.job.maps";}\n')
(src/'DataDrivenDBInputFormat.java').write_text('package '+package+'; import org.apache.hadoop.mapreduce.InputSplit; public class DataDrivenDBInputFormat { public static class DataDrivenDBInputSplit extends InputSplit { private final String lower,upper; public DataDrivenDBInputSplit(String a,String b){lower=a;upper=b;}public String getLowerClause(){return lower;}public String getUpperClause(){return upper;}public long getLength(){return 0;}public String[] getLocations(){return new String[0];}}}\n')
(src/'SplitBoundaryControl.java').write_text('''package co.nubetech.hiho.mapreduce.lib.db.apache;
import org.apache.hadoop.conf.Configuration;import org.apache.hadoop.mapreduce.InputSplit;import java.sql.ResultSet;import java.util.*;import static org.mockito.Mockito.*;
public class SplitBoundaryControl {
 static int checks;
 static double bound(String s){return Double.parseDouble(s.substring(s.lastIndexOf(' ')+1));}
 public static void main(String[] args)throws Exception{
  for(int[] test:new int[][]{{0,10,1},{0,10,2},{0,0,1}}){Configuration c=new Configuration();c.setInt(MRJobConfig.NUM_MAPS,test[2]);ResultSet rs=mock(ResultSet.class);when(rs.getString(1)).thenReturn("value");when(rs.getString(2)).thenReturn("value");when(rs.getDouble(1)).thenReturn((double)test[0]);when(rs.getDouble(2)).thenReturn((double)test[1]);List<InputSplit> splits=new FloatSplitter().split(c,rs,"col");List<Integer> missing=new ArrayList<>();
   for(int n=test[0];n<=test[1];n++){boolean found=false;for(InputSplit s:splits){DataDrivenDBInputFormat.DataDrivenDBInputSplit d=(DataDrivenDBInputFormat.DataDrivenDBInputSplit)s;double low=bound(d.getLowerClause()),high=bound(d.getUpperClause());if(n>=low&&(d.getUpperClause().contains(" <= ")?n<=high:n<high))found=true;}if(!found)missing.add(n);}if(missing.isEmpty())throw new AssertionError("expected omission observation");checks++;System.out.println("OBSERVED FloatSplitter range="+test[0]+".."+test[1]+" maps="+test[2]+" missing="+missing);for(InputSplit s:splits){DataDrivenDBInputFormat.DataDrivenDBInputSplit d=(DataDrivenDBInputFormat.DataDrivenDBInputSplit)s;System.out.println(d.getLowerClause()+" AND "+d.getUpperClause());}}
  if(!new IntegerSplitter().split(2,0,10).equals(List.of(0L,5L,10L)))throw new AssertionError();checks++;System.out.println("PASS integer ordinary boundary points");
  ResultSet bool=mock(ResultSet.class);when(bool.getString(1)).thenReturn("false");when(bool.getString(2)).thenReturn("true");when(bool.getBoolean(2)).thenReturn(true);if(new BooleanSplitter().split(new Configuration(),bool,"col").size()!=2)throw new AssertionError();checks++;
  ResultSet nul=mock(ResultSet.class);if(new BooleanSplitter().split(new Configuration(),nul,"col").size()!=1)throw new AssertionError();checks++;System.out.println("PASS boolean ordinary and null controls;checks="+checks);
 }
}
''')
(src/'IntegerOverflowControl.java').write_text('package '+package+'; public class IntegerOverflowControl {public static void main(String[] args)throws Exception {System.out.println("BEGIN singleton Long.MAX_VALUE with heap32MiB");try{new IntegerSplitter().split(1,Long.MAX_VALUE,Long.MAX_VALUE);throw new AssertionError("expected overflow observation");}catch(OutOfMemoryError expected){System.out.println("OBSERVED integer singleton range expands until bounded child heap exhausted");}}}\n')
cp=str(classes)+':'+str(runtime/'jars/*');steps=[]
for args in [[str(j/'bin/javac'),'-cp',cp,'-d',str(classes),*[str(p) for p in sorted(src.glob('*.java'))]],[str(j/'bin/java'),'--add-opens=java.base/java.lang=ALL-UNNAMED','-cp',cp,package+'.SplitBoundaryControl'],[str(j/'bin/java'),'-Xmx32m','-cp',cp,package+'.IntegerOverflowControl']]:
 try:
  p=subprocess.run(args,capture_output=True,text=True,timeout=15);item=dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr)
 except subprocess.TimeoutExpired as e:item=dict(args=args,timeout=True,stdout=str(e.stdout),stderr=str(e.stderr),returncode=None)
 steps.append(item);print(json.dumps(item))
 if item['returncode']!=0:break
out=dict(scope='Unchanged original6source classes; real Hadooptypes/Mockito JDBC ResultSet doubles, authored recording DataDrivenDBInputSplit and MRJobConfig hosts in separate classesdir; no DB/cluster.6normal JVM controls plus1bounded heap child. No full DataDrivenDBInputFormat runtime qualification.',commands=steps)
(root/'docs/research/dedup-500/hiho098-split-native-controls-20261024.json').write_text(json.dumps(out,indent=2)+'\n');assert all(x['returncode']==0 for x in steps)
