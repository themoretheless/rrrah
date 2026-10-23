import co.nubetech.hiho.dedup.*;
import org.apache.hadoop.io.*;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

public class BoundaryControl {
  static int checks;
  static void require(boolean b, String label) { if (!b) throw new AssertionError(label); checks++; System.out.println("PASS " + label); }
  static void invalid(DelimitedLineRecordReader reader, String text, int column) throws Exception {
    try { reader.getColumn(new Text(text),column,","); throw new AssertionError("expected error"); }
    catch (IndexOutOfBoundsException | NegativeArraySizeException expected) { checks++; System.out.println("OBSERVED invalid slice: " + text + " column=" + column + " " + expected.getClass().getSimpleName()); }
  }
  public static void main(String[] args) throws Exception {
    HihoTuple<Text> a=new HihoTuple<>(), b=new HihoTuple<>();
    a.setKey(new Text("AAAA")); b.setKey(new Text("BBBB")); b.setHash(a.getHash());
    require(a.compareTo(b)==0 && !a.equals(b),"equal digest compares0 for distinct keys");
    Text reused=new Text("longer prior content"); reused.set("abc".getBytes(StandardCharsets.UTF_8),0,3); Text fresh=new Text("abc");
    require(reused.equals(fresh),"same logical Text after shrink");
    require(!HashUtility.getMD5Hash(reused).equals(HashUtility.getMD5Hash(fresh)),"direct Text overload hashes spare backing bytes");
    a.setKey(reused); b.setKey(fresh);
    require(a.getHash().equals(b.getHash()),"generic tuple Object overload avoids Text backing-buffer difference");
    ArrayWritable one=new ArrayWritable(new String[]{"same"}), two=new ArrayWritable(new String[]{"same"});
    require(!HashUtility.getMD5Hash(one).equals(HashUtility.getMD5Hash(two)),"independent equal-element arrays get different toString hashes");
    DelimitedLineRecordReader reader=new DelimitedLineRecordReader();
    require(reader.getColumn(new Text("a,b,c"),2,",").equals(new Text("b")),"ASCII middle field control");
    invalid(reader,"a,b,c",3); invalid(reader,"a,b,c",0); invalid(reader,"abc",1);
    Text parsed=reader.getColumn(new Text("a§b§c"),2,"§");
    byte[] bytes=Arrays.copyOf(parsed.getBytes(),parsed.getLength());
    require(!parsed.equals(new Text("b")),"multibyte delimiter corrupts field");
    System.out.println("multibyte field raw="+java.util.HexFormat.of().formatHex(bytes));
    require(reader.getColumn(new Text("a,,c"),2,",").getLength()==0,"empty middle field retained");
    System.out.println("boundary checks="+checks);
  }
}
