// Read screenshots through the system codec; never rewrite source pixels.
import AppKit
import Foundation
let root=URL(fileURLWithPath:CommandLine.arguments[1])
func load(_ name:String)->NSBitmapImageRep { let image=NSBitmapImageRep(data:try! Data(contentsOf:root.appendingPathComponent(name)))!;precondition(image.bitsPerSample==8 && !image.isPlanar && image.samplesPerPixel>=3);return image }
func diff(_ a:NSBitmapImageRep,_ b:NSBitmapImageRep,_ rect:[Int])->[String:Any] {
    precondition(a.pixelsWide==b.pixelsWide && a.pixelsHigh==b.pixelsHigh)
    var histogram=[Int](repeating:0,count:256);var sum=0;var count=0
    for y in rect[1]..<rect[3] {for x in rect[0]..<rect[2] {var d=0;for c in 0..<3 {let i=y*a.bytesPerRow+x*(a.bitsPerPixel/8)+c;let j=y*b.bytesPerRow+x*(b.bitsPerPixel/8)+c;d=max(d,abs(Int(a.bitmapData![i])-Int(b.bitmapData![j])))};histogram[d]+=1;sum+=d;count+=1}}
    var cumulative=0;var p99=0;for i in 0..<256 {cumulative+=histogram[i];if cumulative*100>=count*99 {p99=i;break}}
    return ["pixels":count,"max_channel_difference":histogram.lastIndex(where:{$0>0})!,"p99_max_channel_difference":p99,"mean_max_channel_difference":Double(sum)/Double(count)]
}
var out=[String:Any]()
for (key,before,after,split) in [("initial","comparison-original.png","comparison-edited.png","comparison-split.png"),("pan_zoom","comparison-pan-before.png","comparison-pan-edited.png","comparison-pan-split.png")] {
    let a=load(before),b=load(after),c=load(split);out[key]=["left":diff(a,c,[100,150,700,500]),"right":diff(b,c,[900,150,1500,500])]
}
out["scope"]="Native screenshot transport is JPEG despite .png names; approximate visual comparison only, exact pixels qualified separately by Metal readback."
let result=try! JSONSerialization.data(withJSONObject:out,options:[.prettyPrinted,.sortedKeys]);try! result.write(to:root.appendingPathComponent("comparison-pixels.json"));print(String(data:result,encoding:.utf8)!)

for name in ["comparison-original.png","comparison-edited.png","comparison-split.png","comparison-pan-before.png","comparison-pan-edited.png","comparison-pan-split.png"] {let a=load(name);var row=[String]();for x in [200,700,790,799,800,810,1000,1400] {let i=300*a.bytesPerRow+x*(a.bitsPerPixel/8);row.append("\(x):\(a.bitmapData![i]),\(a.bitmapData![i+1]),\(a.bitmapData![i+2])")};print(name,row)}
