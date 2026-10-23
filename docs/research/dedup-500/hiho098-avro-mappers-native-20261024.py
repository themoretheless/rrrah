from pathlib import Path
import json,subprocess,shutil
root=Path.cwd();runtime=root/'.dedup-work/hiho098-runtime-20261024';j=next(runtime.glob('*/Contents/Home'));b=root/'.dedup-work/hiho098-format-runtime-20261024';src=b/'src';classes=b/'classes';f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';sources=[]
for n in [139,140,141,142]:
 p=next(f.glob(str(n)+'-*.java'));q=src/p.name.split('-',1)[1];shutil.copyfile(p,q);sources.append(str(q))
(src/'AvroMapperBoundaryControl.java').write_text('''import co.nubetech.hiho.avro.DBMapper;import co.nubetech.hiho.mapreduce.DBInputAvroMapper;import co.nubetech.hiho.mapreduce.lib.db.*;import org.apache.avro.Schema;import org.apache.avro.generic.*;import org.apache.avro.io.BinaryEncoder;import java.util.*;import java.sql.Types;import java.io.*;
public class AvroMapperBoundaryControl{
 static GenericRecord value(int type,Object x){ArrayList<ColumnInfo> cols=new ArrayList<>();cols.add(new ColumnInfo(0,type,"value"));return new DBInputAvroMapper().getValueRecord(DBMapper.getSchema(cols),new GenericDBWritable(cols,new ArrayList<>(Arrays.asList(x))));}
 static void encode(GenericRecord r)throws Exception{new GenericDatumWriter<GenericRecord>(r.getSchema()).write(r,new BinaryEncoder(new ByteArrayOutputStream()));}
 public static void main(String[] args)throws Exception{
  GenericRecord text=value(Types.VARCHAR,"owned");encode(text);if(!text.get(0).toString().equals("owned"))throw new AssertionError();System.out.println("PASS nonnull string converted and encoded");
  try{value(Types.VARCHAR,null);throw new AssertionError();}catch(NullPointerException expected){System.out.println("OBSERVED null VARCHAR throws during toString conversion");}
  try{DBMapper.getSchema(Types.ARRAY);throw new AssertionError();}catch(org.apache.avro.AvroRuntimeException expected){System.out.println("OBSERVED production ARRAY schema rejected");}
  GenericRecord d=value(Types.DOUBLE,84.5d);encode(d);if(!GenericData.get().validate(d.getSchema(),d))throw new AssertionError();System.out.println("PASS Double schema value validated and encoded");
  GenericRecord single=value(Types.FLOAT,1.0f);if(single.getSchema().getFields().get(0).schema().getType()!=Schema.Type.DOUBLE||GenericData.get().validate(single.getSchema(),single))throw new AssertionError();try{encode(single);throw new AssertionError("expected Avro1.4.1 type mismatch");}catch(ClassCastException expected){System.out.println("OBSERVED SQL FLOAT retained as JavaFloat under DOUBLEschema, fails actualAvroencoding");}System.out.println("Avro mapper checks=5");
 }
}
''')
cp=str(classes)+':'+str(runtime/'jars/*');steps=[]
for i,args in enumerate([[str(j/'bin/javac'),'-cp',cp,'-d',str(classes),*sources,str(src/'AvroMapperBoundaryControl.java')],[str(j/'bin/java'),'--add-opens=java.base/java.lang=ALL-UNNAMED','-cp',cp,'org.junit.runner.JUnitCore','co.nubetech.hiho.avro.TestDBMapper','co.nubetech.hiho.mapreduce.TestDBInputAvroMapper'],[str(j/'bin/java'),'-cp',cp,'AvroMapperBoundaryControl']]):
 p=subprocess.run(args,capture_output=True,text=True,timeout=30);steps.append(dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr));print(p.stdout,p.stderr)
 if i==0 and p.returncode:break
(root/'docs/research/dedup-500/hiho098-avro-mappers-native-controls-20261024.json').write_text(json.dumps(dict(scope='6unchangedoriginalJUnitmethods plus5authorcontrols; actualAvro1.4.1/Jackson1.4.0/Hadoop, owned memory encoding; no database/network/cluster; original failures preserved if any',commands=steps),indent=2)+'\n');assert len(steps)==3 and steps[0]['returncode']==0 and steps[2]['returncode']==0
