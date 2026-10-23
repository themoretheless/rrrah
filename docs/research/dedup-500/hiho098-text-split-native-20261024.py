from pathlib import Path
import subprocess,json,shutil
root=Path.cwd();runtime=root/'.dedup-work/hiho098-runtime-20261024';j=next(runtime.glob('*/Contents/Home'));base=root/'.dedup-work/hiho098-split-runtime-20261024';src=base/'src';classes=base/'classes';f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';shutil.copyfile(f/'111-TextSplitter.java',src/'TextSplitter.java')
(src/'TextSplitControl.java').write_text('''package co.nubetech.hiho.mapreduce.lib.db.apache;
import java.sql.ResultSet;import java.util.*;import org.apache.hadoop.conf.Configuration;import org.apache.hadoop.mapreduce.InputSplit;import static org.mockito.Mockito.*;
public class TextSplitControl{public static void main(String[] args)throws Exception{
 TextSplitter t=new TextSplitter();if(!t.bigDecimalToString(t.stringToBigDecimal("abc")).equals("abc"))throw new AssertionError();System.out.println("PASS ASCII text conversion roundtrip");
 String emoji="\\ud83d\\ude00x",decoded=t.bigDecimalToString(t.stringToBigDecimal(emoji));if(decoded.equals(emoji))throw new AssertionError();System.out.println("OBSERVED supplementary text roundtrip differs: UTF16length="+emoji.length()+" -> "+decoded.length());
 String nul=t.bigDecimalToString(t.stringToBigDecimal("a\\u0000b"));if(!nul.equals("a"))throw new AssertionError();System.out.println("OBSERVED embedded NUL terminates converted suffix");
 ResultSet rs=mock(ResultSet.class);when(rs.getString(1)).thenReturn("O'Neil");when(rs.getString(2)).thenReturn("O'Neil");List<InputSplit> splits=t.split(new Configuration(),rs,"col");DataDrivenDBInputFormat.DataDrivenDBInputSplit s=(DataDrivenDBInputFormat.DataDrivenDBInputSplit)splits.get(0);if(!s.getLowerClause().equals("col >= 'O'Neil'"))throw new AssertionError();System.out.println("OBSERVED ordinary apostrophe name emitted without SQL-literal escaping: "+s.getLowerClause());System.out.println("text split checks=4");}}
''')
cp=str(classes)+':'+str(runtime/'jars/*');steps=[]
for args in [[str(j/'bin/javac'),'-cp',cp,'-d',str(classes),str(src/'TextSplitter.java'),str(src/'TextSplitControl.java')],[str(j/'bin/java'),'--add-opens=java.base/java.lang=ALL-UNNAMED','-cp',cp,'co.nubetech.hiho.mapreduce.lib.db.apache.TextSplitControl']]:
 p=subprocess.run(args,capture_output=True,text=True,timeout=15);steps.append(dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr));print(p.stdout,p.stderr)
 if p.returncode:break
(root/'docs/research/dedup-500/hiho098-text-split-native-controls-20261024.json').write_text(json.dumps(dict(scope='Original TextSplitter conversion and clause generation with actual Hadooptypes/Mockito and previous recording dependencyhosts;4authorchecks; no database/SQL execution or full import; helper roundtrip differences alone do not prove missing output ranges because endpoints are reinserted',commands=steps),indent=2)+'\n');assert all(x['returncode']==0 for x in steps)
