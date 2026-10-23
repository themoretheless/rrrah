from pathlib import Path
import subprocess,json,shutil
root=Path.cwd();runtime=root/'.dedup-work/hiho098-runtime-20261024';j=next(runtime.glob('*/Contents/Home'));base=root/'.dedup-work/hiho098-format-runtime-20261024';src=base/'src';classes=base/'classes';f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';shutil.copyfile(f/'122-DBOutputFormat.java',src/'DBOutputFormat.java')
(src/'WriterBoundaryControl.java').write_text('''package co.nubetech.hiho.mapreduce.lib.db.apache;
import java.sql.*;import java.io.*;import static org.mockito.Mockito.*;
public class WriterBoundaryControl{
 static class Fixture{Connection conn=mock(Connection.class);PreparedStatement stmt=mock(PreparedStatement.class);DBWritable key=mock(DBWritable.class);DBOutputFormat<DBWritable,Object> format=new DBOutputFormat<>();DBOutputFormat<DBWritable,Object>.DBRecordWriter writer;Fixture()throws Exception{writer=format.new DBRecordWriter(conn,stmt);}}
 public static void main(String[] args)throws Exception{
  Fixture good=new Fixture();good.writer.write(good.key,null);good.writer.close(null);verify(good.key).write(good.stmt);verify(good.stmt).addBatch();verify(good.stmt).executeBatch();verify(good.conn).commit();verify(good.stmt).close();verify(good.conn).close();System.out.println("PASS healthy bind,batch,commit,close");
  Fixture bind=new Fixture();doThrow(new SQLException("owned bind failure")).when(bind.key).write(bind.stmt);bind.writer.write(bind.key,null);verify(bind.stmt,never()).addBatch();System.out.println("OBSERVED binding SQLException swallowed by write");
  Fixture add=new Fixture();doThrow(new SQLException("owned addBatch failure")).when(add.stmt).addBatch();add.writer.write(add.key,null);verify(add.conn,never()).rollback();System.out.println("OBSERVED addBatch SQLException swallowed by write");
  Fixture batch=new Fixture();doThrow(new SQLException("owned batch failure")).when(batch.stmt).executeBatch();try{batch.writer.close(null);throw new AssertionError();}catch(IOException e){if(!e.getMessage().equals("owned batch failure"))throw new AssertionError(e);}verify(batch.conn).rollback();verify(batch.conn).close();System.out.println("PASS executeBatch failure propagates with rollback+close");
  Fixture mask=new Fixture();doThrow(new SQLException("owned primary batch failure")).when(mask.stmt).executeBatch();doThrow(new SQLException("owned cleanup failure")).when(mask.stmt).close();try{mask.writer.close(null);throw new AssertionError();}catch(IOException e){if(!e.getMessage().equals("owned cleanup failure"))throw new AssertionError(e);}verify(mask.conn).rollback();verify(mask.conn,never()).close();System.out.println("OBSERVED cleanup exception masks batch failure and skips connectionclose");
  Fixture commit=new Fixture();doThrow(new SQLException("owned commit failure")).when(commit.conn).commit();try{commit.writer.close(null);throw new AssertionError();}catch(IOException expected){}verify(commit.conn).rollback();verify(commit.stmt).close();verify(commit.conn).close();System.out.println("PASS commit failure triggers rollback and closes resources;writer checks=6");
 }
}
''')
cp=str(classes)+':'+str(runtime/'jars/*');steps=[]
for args in [[str(j/'bin/javac'),'-cp',cp,'-d',str(classes),str(src/'DBOutputFormat.java'),str(src/'WriterBoundaryControl.java')],[str(j/'bin/java'),'--add-opens=java.base/java.lang=ALL-UNNAMED','-cp',cp,'co.nubetech.hiho.mapreduce.lib.db.apache.WriterBoundaryControl']]:
 p=subprocess.run(args,capture_output=True,text=True,timeout=30);steps.append(dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr));print(p.stdout,p.stderr)
 if p.returncode:break
(root/'docs/research/dedup-500/hiho098-db-writer-native-controls-20261024.json').write_text(json.dumps(dict(scope='6authorcontrols of unchanged original DBOutputFormat.DBRecordWriter with Mockito Connection/PreparedStatement/DBWritable, actual Java/Hadoop classes. No JDBCdriver connection, actual SQL/write, external DB orcluster.',commands=steps),indent=2)+'\n');assert all(x['returncode']==0 for x in steps)
