import Foundation
@main struct IntensityProbe {
 static func main() {
 var inversions = 0
 var checked = 0
 for count in 1...128 {
 for mode in ScanIntensity.allCases {
 let nominal = [mode.metadataConcurrencyLimit(processorCount: count, thermalState: .nominal), mode.hashConcurrencyLimit(processorCount: count, thermalState: .nominal), mode.comparisonConcurrencyLimit(processorCount: count, thermalState: .nominal), mode.visionFeatureConcurrencyLimit(thermalState: .nominal)]
 let fair = [mode.metadataConcurrencyLimit(processorCount: count, thermalState: .fair), mode.hashConcurrencyLimit(processorCount: count, thermalState: .fair), mode.comparisonConcurrencyLimit(processorCount: count, thermalState: .fair), mode.visionFeatureConcurrencyLimit(thermalState: .fair)]
 let serious = [mode.metadataConcurrencyLimit(processorCount: count, thermalState: .serious), mode.hashConcurrencyLimit(processorCount: count, thermalState: .serious), mode.comparisonConcurrencyLimit(processorCount: count, thermalState: .serious), mode.visionFeatureConcurrencyLimit(thermalState: .serious)]
 let critical = [mode.metadataConcurrencyLimit(processorCount: count, thermalState: .critical), mode.hashConcurrencyLimit(processorCount: count, thermalState: .critical), mode.comparisonConcurrencyLimit(processorCount: count, thermalState: .critical), mode.visionFeatureConcurrencyLimit(thermalState: .critical)]
 for i in 0..<4 {
 checked += 1
 precondition(nominal[i] >= 1 && nominal[i] <= 8)
 precondition(fair[i] >= 1 && fair[i] <= nominal[i])
 precondition(serious[i] >= 1 && serious[i] <= nominal[i])
 precondition(critical[i] == 1)
 if serious[i] > fair[i] { inversions += 1 }
 }
 }
 }
 precondition(inversions > 0)
 precondition(ScanIntensity.fast.visionFeatureConcurrencyLimit(thermalState: .fair) == 1)
 precondition(ScanIntensity.fast.visionFeatureConcurrencyLimit(thermalState: .serious) == 2)
 print("mode_processor_stage_cases=\(checked),fair_to_serious_increases=\(inversions)")
 print("fast_vision_fair=1,fast_vision_serious=2,all_limits_within_nominal_and_critical_one=true")
 }
}
