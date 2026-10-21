func fixture(_ stride: Int) -> [Float] {
 let data = UnsafeMutablePointer<UInt8>.allocate(capacity: 32 * stride)
 data.initialize(repeating: 251, count: 32 * stride)
 defer { data.deallocate() }
 for y in 0..<32 { for x in 0..<32 { data[y*stride+x] = UInt8((x*7+y*13)%239) } }
 let buffer = vImage_Buffer(data: data, height: 32, width: 32, rowBytes: stride)
 return apply2DDCT(to: buffer)!
}
let tight = fixture(32)
let padded = fixture(40)
precondition(tight.count == 64 && padded.count == 64)
precondition(tight != padded)
print("PASS original DCT counterexample: identical logical pixels with rowBytes32/40 produce different coefficients")
print("max absolute coefficient difference: \(zip(tight,padded).map { abs($0-$1) }.max()!)")
