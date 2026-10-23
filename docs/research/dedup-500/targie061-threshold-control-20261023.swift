import Foundation
@main struct ThresholdProbe {
 static func main() {
 let cases: [(String, Double)] = [("1%", 1), ("0.88%", 0.88), ("1.01%", 0.60), ("8%8", 0.88), ("88%%", 0.88)]
 for (text, expected) in cases {
 let actual = DisplayThresholdEditing.thresholdValue(from: text)!
 precondition(abs(actual - expected) < 1e-12)
 print("input=\(text),threshold=\(actual)")
 }
 for invalid in ["nan", "inf", "-inf", "", "abc"] {
 precondition(DisplayThresholdEditing.thresholdValue(from: invalid) == nil)
 }
 precondition(DisplayThresholdEditing.sliderValue(for: 0.85) == 0.88)
 precondition(DisplayThresholdEditing.thresholdValue(from: "85") == 0.85)
 print("nonfinite_rejected_and_manual_vs_slider_distinct=true")
 }
}
