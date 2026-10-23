import co.nubetech.hiho.common.OutputStrategyEnum;
import java.util.Locale;
public class StrategyLocaleControl{
 public static void main(String[] args){Locale prior=Locale.getDefault();try{
  Locale.setDefault(Locale.ENGLISH);if(OutputStrategyEnum.value("delimited")!=OutputStrategyEnum.DELIMITED)throw new AssertionError();System.out.println("PASS English lowercase strategy");
  Locale.setDefault(Locale.forLanguageTag("tr-TR"));try{OutputStrategyEnum.value("delimited");throw new AssertionError("expected Turkish observation");}catch(IllegalArgumentException expected){System.out.println("OBSERVED Turkish lowercase strategy rejected: "+expected.getMessage());}
  if(OutputStrategyEnum.value("DELIMITED")!=OutputStrategyEnum.DELIMITED)throw new AssertionError();System.out.println("PASS Turkish uppercase strategy");if(OutputStrategyEnum.value(null)!=null)throw new AssertionError();System.out.println("PASS null strategy remains null;checks=4");
 }finally{Locale.setDefault(prior);}}
}
