import Foundation
let input = (0..<1024).map { Float($0 % 17) / 40 + 0.4 }
let output = Probe.applyCLAHE(input,size:32)
precondition(Set(input).count > 1 && output.allSatisfy { $0 == 1 })
print("PASS default32 CLAHE maps distinct moderate-bright input values entirely to1: 4x4tile/64bins gives integer clipValue0")
let bigger = (0..<4096).map { Float($0 % 17) / 40 + 0.4 }
precondition(Set(Probe.applyCLAHE(bigger,size:64)).count > 1)
print("PASS size64 positive control preserves multiple output levels")
let zero=Bitset256();let ones=Bitset256(.max,.max,.max,.max)
let tied=FuzzyTemplate.build(from:[zero,ones]);precondition(tied.template==ones && tied.confidence.allSatisfy{$0==1})
print("PASS documented tie policy selects1 at all256positions")
let unanimous=FuzzyTemplate.build(from:[zero,zero,zero]);precondition(unanimous.template==zero && unanimous.confidence.allSatisfy{$0==0})
print("PASS unanimous zero-bit agreement reports confidence0: scores count ones, not agreement with consensus")
print("RESULT 4 passed; original CLAHE body/access adaptation and unchanged template/bitset; no full CGImage pipeline run")
