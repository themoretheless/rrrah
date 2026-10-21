import Foundation
for bit in 0..<256 {
 var bits = [Bool](repeating:false,count:256);bits[bit]=true
 let packed = Bitset256.fromBits(bits)
 for index in 0..<256 { precondition(packed.bit(at:index) == (index==bit)) }
 precondition(packed.hammingDistance(Bitset256()) == 1)
}
print("PASS all 256 one-hot signatures round-trip with unit Hamming distance")
precondition(Bitset256().hammingDistance(Bitset256(.max,.max,.max,.max)) == 256)
print("PASS full-radius distance 256")
let decoded = try JSONDecoder().decode(Bitset256.self,from:Data("[1,2,3,4,999]".utf8))
precondition(decoded == Bitset256(1,2,3,4))
print("PASS decoder accepts trailing fifth word and ignores it; strict wire-length validation absent")
precondition(Bitset256(0,0,0,1).bit(at:256))
print("PASS index256 reads fourth-word bit0; documented index range not runtime validated")
print("RESULT 4 controls passed; unchanged original Bitset256 compiled with Swift, no image or authentication pipeline execution")
