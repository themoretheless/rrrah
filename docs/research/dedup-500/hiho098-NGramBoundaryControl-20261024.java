import co.nubetech.hiho.similarity.ngram.*;
import org.apache.hadoop.io.*;
import org.apache.hadoop.mapreduce.*;
import org.mockito.*;
import static org.mockito.Mockito.*;
import java.util.*;
public class NGramBoundaryControl {
 static int checks;
 static void require(boolean ok,String label){if(!ok)throw new AssertionError(label);checks++;System.out.println("PASS "+label);}
 public static void main(String[] args)throws Exception{
  NGramMapper mapper=new NGramMapper();Mapper.Context shortContext=mock(Mapper.Context.class);mapper.map(new Text("single"),new Text("id"),shortContext);verifyZeroInteractions(shortContext);require(mapper.getNGrams(new Text("single"),2).isEmpty(),"one-token input emits no candidate postings");
  Mapper.Context positive=mock(Mapper.Context.class);mapper.map(new Text("common gram"),new Text("id"),positive);verify(positive).write(new Text("common gram"),new Text("common gramdelimiterBetweenKeyAndValueid"));checks++;System.out.println("PASS two-token posting control");
  String marker="delimiterBetweenKeyAndValue";Mapper.Context collisions=mock(Mapper.Context.class);mapper.map(new Text("common gram tail"),new Text("foo"+marker+"bar"),collisions);mapper.map(new Text("common gram tail"+marker+"foo"),new Text("bar"),collisions);
  ArgumentCaptor<Text> packed=ArgumentCaptor.forClass(Text.class);verify(collisions,times(2)).write(eq(new Text("common gram")),packed.capture());require(packed.getAllValues().get(0).equals(packed.getAllValues().get(1)),"distinct key/value pairs collapse to identical sentinel-packed posting");
  NGramReducer reducer=new NGramReducer();Reducer.Context left=mock(Reducer.Context.class),right=mock(Reducer.Context.class);reducer.reduce(new Text("gram"),List.of(new Text("A"),new Text("B")),left);reducer.reduce(new Text("gram"),List.of(new Text("B"),new Text("A")),right);ArgumentCaptor<ValuePair> a=ArgumentCaptor.forClass(ValuePair.class),b=ArgumentCaptor.forClass(ValuePair.class);verify(left).write(a.capture(),eq(new IntWritable(1)));verify(right).write(b.capture(),eq(new IntWritable(1)));require(!a.getValue().equals(b.getValue())&&a.getValue().compareTo(b.getValue())!=0,"reversed candidate order produces distinct score keys");
  System.out.println("ngram boundary checks="+checks);
 }
}
