// Read-only macOS EDR capability snapshot. Run on the main thread:
//   swift scripts/inspect-metal-display.swift
// Repeat with the HDR viewer visible to compare current dynamic headroom.
// These OS values do not measure physical luminance or color accuracy.
import AppKit
import Foundation

struct DisplaySnapshot: Codable {
    let name: String
    let currentEDR: Double
    let potentialEDR: Double
    let referenceEDR: Double
}
struct Snapshot: Codable {
    let schemaVersion: Int
    let observedAt: String
    let displays: [DisplaySnapshot]
}

let snapshot = Snapshot(
    schemaVersion: 1,
    observedAt: ISO8601DateFormatter().string(from: Date()),
    displays: NSScreen.screens.map { screen in
        DisplaySnapshot(
            name: screen.localizedName,
            currentEDR: Double(screen.maximumExtendedDynamicRangeColorComponentValue),
            potentialEDR: Double(screen.maximumPotentialExtendedDynamicRangeColorComponentValue),
            referenceEDR: Double(screen.maximumReferenceExtendedDynamicRangeColorComponentValue)
        )
    }
)
let encoder = JSONEncoder()
encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
let data = try encoder.encode(snapshot)
FileHandle.standardOutput.write(data)
FileHandle.standardOutput.write(Data("\n".utf8))
