import AppKit
import Metal
import Foundation
let screens: [[String: Any]] = NSScreen.screens.map { screen in
    ["name": screen.localizedName,
     "frame": [screen.frame.width, screen.frame.height],
     "color_space": screen.colorSpace?.localizedName ?? "unknown",
     "edr_current_max": screen.maximumExtendedDynamicRangeColorComponentValue,
     "edr_potential_max": screen.maximumPotentialExtendedDynamicRangeColorComponentValue,
     "edr_reference_max": screen.maximumReferenceExtendedDynamicRangeColorComponentValue]
}
let device = MTLCreateSystemDefaultDevice()
let result: [String: Any] = ["metal_device": device?.name ?? "none", "screens": screens,
    "scope": "read-only live NSScreen EDR capability/headroom; not measured luminance or calibration"]
let data = try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys])
print(String(data: data, encoding: .utf8)!)
