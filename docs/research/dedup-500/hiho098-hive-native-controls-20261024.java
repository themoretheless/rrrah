import co.nubetech.hiho.hive.HiveUtility;
import co.nubetech.hiho.common.HIHOConf;
import org.apache.hadoop.conf.Configuration;
import java.nio.file.*;
import java.io.*;
import org.junit.Test;
import static org.junit.Assert.*;
public class HiveControls {
 @Test public void appendPathDiffers() throws Throwable {
 Path dir=Files.createTempDirectory("hiho-hive-owned-");
 Configuration c=new Configuration();c.set(HIHOConf.INPUT_OUTPUT_LOADTO_PATH,dir.toString());
 try(FileOutputStream f=HiveUtility.getFileOutputStream(c,0,"t")){f.write(65);}
 Path sibling=Paths.get(dir.toString()+"hiveScriptt.txt");
 try {try(FileOutputStream f=HiveUtility.getFileOutputStream(c,1,"t")){f.write(66);}
 assertEquals("A",Files.readString(dir.resolve("hiveScriptt.txt")));assertEquals("B",Files.readString(sibling));
 }finally{Files.deleteIfExists(sibling);Files.deleteIfExists(dir.resolve("hiveScriptt.txt"));Files.delete(dir);}
 }
 @Test public void missingDriverSwallowed() throws Throwable {
 Configuration c=new Configuration();c.set(HIHOConf.HIVE_DRIVER,"owned.nonexistent.Driver");
 HiveUtility.runQuery("owned-create","owned-load",true,c);
 }
 @Test public void multipleDynamicKeepsLast() {assertEquals("b",HiveUtility.getDynamicPartitionBy("a:string;b:string"));}
 @Test public void semicolonMismatch() throws Throwable {
 Configuration c=new Configuration();c.set(HIHOConf.HIVE_TABLE_NAME,"t");
 String out=HiveUtility.getInsertQueryFromTmpToMain(c,new co.nubetech.hiho.mapreduce.lib.db.GenericDBWritable(new java.util.ArrayList<co.nubetech.hiho.mapreduce.lib.db.ColumnInfo>(),null),"a:string:x;b:string:y");
 assertTrue(out.contains("a='x;b'"));assertFalse(out.contains("b='y'"));
 }
}
