from pathlib import Path
import subprocess,json,shutil
root=Path.cwd();runtime=root/'.dedup-work/hiho098-runtime-20261024';j=next(runtime.glob('*/Contents/Home'));base=root/'.dedup-work/hiho098-split-runtime-20261024';src=base/'src';classes=base/'classes';f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';package='co.nubetech.hiho.mapreduce.lib.db.apache';sources=[]
for x in json.loads((root/'.dedup-work/hiho098-decimal-date-files-20261024.json').read_text()):
 p=src/x['local'].split('-',1)[1];shutil.copyfile(f/x['local'],p);sources.append(str(p))
(src/'DecimalDateControl.java').write_text('''package co.nubetech.hiho.mapreduce.lib.db.apache;
import java.math.BigDecimal;import java.sql.*;import java.util.*;import org.apache.hadoop.conf.Configuration;import org.apache.hadoop.mapreduce.InputSplit;import static org.mockito.Mockito.*;
public class DecimalDateControl{
 public static void main(String[] args)throws Exception{
  BigDecimalSplitter b=new BigDecimalSplitter();if(!b.split(new BigDecimal(2),BigDecimal.ZERO,BigDecimal.ONE).equals(List.of(BigDecimal.ZERO,new BigDecimal("0.5"),new BigDecimal("1.0"))))throw new AssertionError();System.out.println("PASS decimal halves cover0..1");
  if(b.tryDivide(BigDecimal.ONE,new BigDecimal(3)).compareTo(BigDecimal.ZERO)!=0)throw new AssertionError();System.out.println("OBSERVED integer-scale1/3 rounds to0, triggering MIN_INCREMENT fallback");
  ResultSetMetaData meta=mock(ResultSetMetaData.class);when(meta.getColumnType(1)).thenReturn(Types.TIMESTAMP);ResultSet nul=mock(ResultSet.class);when(nul.getMetaData()).thenReturn(meta);List<InputSplit> ns=new DateSplitter().split(new Configuration(),nul,"col");DataDrivenDBInputFormat.DataDrivenDBInputSplit n=(DataDrivenDBInputFormat.DataDrivenDBInputSplit)ns.get(0);if(ns.size()!=1||!n.getLowerClause().equals("col IS NULL"))throw new AssertionError();System.out.println("PASS null timestamp split");
  Timestamp lo=new Timestamp(0),hi=new Timestamp(0);lo.setNanos(1);hi.setNanos(999999);ResultSet rs=mock(ResultSet.class);when(rs.getMetaData()).thenReturn(meta);when(rs.getTimestamp(1)).thenReturn(lo);when(rs.getTimestamp(2)).thenReturn(hi);List<InputSplit> splits=new DateSplitter().split(new Configuration(),rs,"col");DataDrivenDBInputFormat.DataDrivenDBInputSplit s=(DataDrivenDBInputFormat.DataDrivenDBInputSplit)splits.get(0);if(splits.size()!=1||!s.getLowerClause().contains(".000000001")||!s.getUpperClause().contains(".000999999"))throw new AssertionError();System.out.println("PASS submillisecond endpoints preserved: "+s.getLowerClause()+" AND "+s.getUpperClause());System.out.println("decimal/date regular checks=4");
 }
}
''')
(src/'DecimalGrowthControl.java').write_text('package '+package+';import java.math.BigDecimal;public class DecimalGrowthControl{public static void main(String[] args)throws Exception{System.out.println("BEGIN decimal0..1 over3splits with32MiB heap");try{new BigDecimalSplitter().split(new BigDecimal(3),BigDecimal.ZERO,BigDecimal.ONE);throw new AssertionError("expected tiny-step growth observation");}catch(OutOfMemoryError expected){System.out.println("OBSERVED integer-scale decimal range grows until bounded child heap exhausted");}}}\n')
cp=str(classes)+':'+str(runtime/'jars/*');steps=[]
for args in [[str(j/'bin/javac'),'-cp',cp,'-d',str(classes),*sources,str(src/'DecimalDateControl.java'),str(src/'DecimalGrowthControl.java')],[str(j/'bin/java'),'--add-opens=java.base/java.lang=ALL-UNNAMED','-cp',cp,package+'.DecimalDateControl'],[str(j/'bin/java'),'-Xmx32m','-cp',cp,package+'.DecimalGrowthControl']]:
 try:
  p=subprocess.run(args,capture_output=True,text=True,timeout=15);item=dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr)
 except subprocess.TimeoutExpired as e:item=dict(args=args,returncode=None,timeout=True,stdout=str(e.stdout),stderr=str(e.stderr))
 steps.append(item);print(json.dumps(item))
 if item['returncode']!=0:break
(root/'docs/research/dedup-500/hiho098-decimal-date-native-controls-20261024.json').write_text(json.dumps(dict(scope='Original decimal/date methods; real Java/Hadoop/Mockito types, previous isolated recording hosts;4regular assertions+1heap-bounded child;noSQL/DB/import or cluster',commands=steps),indent=2)+'\n');assert all(x['returncode']==0 for x in steps)
