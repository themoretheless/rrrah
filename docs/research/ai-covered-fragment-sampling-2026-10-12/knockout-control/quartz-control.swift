import Foundation
import CoreGraphics
let root = CommandLine.arguments[1]
for name in ["ko-all", "ko-last", "nonko-all"] {
 let doc=CGPDFDocument(URL(fileURLWithPath: root+"/"+name+".pdf") as CFURL)!
 var bytes=[UInt8](repeating:0,count:8*8*4)
 let result=bytes.withUnsafeMutableBytes { data -> [UInt8] in
  let ctx=CGContext(data:data.baseAddress,width:8,height:8,bitsPerComponent:8,bytesPerRow:32,space:CGColorSpace(name:CGColorSpace.sRGB)!,bitmapInfo:CGImageAlphaInfo.premultipliedLast.rawValue)!
  ctx.setFillColor(red:1,green:1,blue:1,alpha:1);ctx.fill(CGRect(x:0,y:0,width:8,height:8))
  ctx.drawPDFPage(doc.page(at:1)!)
  return Array(data[(4*8+4)*4..<(4*8+4)*4+4])
 }
 print(name,result)
}
