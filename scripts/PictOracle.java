import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.io.*;
public class PictOracle {
 public static void main(String[] args) throws Exception {
  BufferedImage image=ImageIO.read(new File(args[0]));
  if(image==null)throw new IOException("No PICT reader");
  try(OutputStream out=new FileOutputStream(args[1])) {
   for(int y=0;y<image.getHeight();y++)for(int x=0;x<image.getWidth();x++) {
    int p=image.getRGB(x,y);out.write(p>>16&255);out.write(p>>8&255);out.write(p&255);out.write(p>>>24);
   }
  }
  System.out.println(image.getWidth()+"x"+image.getHeight());
 }
}
