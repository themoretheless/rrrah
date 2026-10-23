import co.nubetech.hiho.merge.HihoValue;
import org.apache.hadoop.io.Text;
import java.io.*;
public class ValueBoundaryControl {
 static int checks;
 static void require(boolean b,String s){if(!b)throw new AssertionError(s);checks++;System.out.println("PASS "+s);}
 static byte[] invalid()throws Exception{
  ByteArrayOutputStream buffer=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(buffer);
  out.writeBoolean(false);Text.writeString(out,"owned.nonexistent.ValueClass");new Text("after").write(out);return buffer.toByteArray();
 }
 public static void main(String[] args)throws Exception{
  HihoValue<Text> a=new HihoValue<>();a.setVal(new Text("before"));a.setIsOld(true);
  ByteArrayOutputStream bytes=new ByteArrayOutputStream();a.write(new DataOutputStream(bytes));
  HihoValue<Text> restored=new HihoValue<>();restored.readFields(new DataInputStream(new ByteArrayInputStream(bytes.toByteArray())));
  require(a.equals(restored),"valid serialization roundtrip");
  a.readFields(new DataInputStream(new ByteArrayInputStream(invalid())));
  require(a.getVal().equals(new Text("after"))&&!a.getIsOld(),"unknown serialized class silently accepted using stale prior value");
  try{new HihoValue<Text>().readFields(new DataInputStream(new ByteArrayInputStream(invalid())));throw new AssertionError("expected error");}
  catch(NullPointerException expected){checks++;System.out.println("OBSERVED fresh unknown-class read gives NullPointerException");}
  System.out.println("checks="+checks);
 }
}
