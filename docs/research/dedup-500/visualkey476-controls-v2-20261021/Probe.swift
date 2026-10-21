import Foundation
// Original CLAHE body, private access removed for isolated authored controls.
enum Probe {
    static func applyCLAHE(_ floats: [Float], size: Int, clipLimit: Float = 2.0) -> [Float] {
        // Determine tile size (8×8 tiles works well for images 64×64 and larger)
        let tileSize = max(size / 8, 4)  // Minimum 4×4 tiles for small images
        let numTilesPerSide = size / tileSize

        guard numTilesPerSide > 0 else { return floats }

        var output = floats
        let numBins = 64  // More bins = smoother results, fewer bins = faster processing

        // Process each tile independently
        for tileY in 0..<numTilesPerSide {
            for tileX in 0..<numTilesPerSide {
                // Step 1: Extract tile pixels
                var tile = [Float]()
                tile.reserveCapacity(tileSize * tileSize)

                for y in 0..<tileSize {
                    for x in 0..<tileSize {
                        let globalX = tileX * tileSize + x
                        let globalY = tileY * tileSize + y

                        // Handle edge cases where image size isn't perfectly divisible by tileSize
                        guard globalY < size && globalX < size else { continue }

                        let idx = globalY * size + globalX
                        tile.append(floats[idx])
                    }
                }

                guard !tile.isEmpty else { continue }

                // Step 2: Build histogram (64 bins covering [0, 1] range)
                var histogram = [Int](repeating: 0, count: numBins)
                for val in tile {
                    // Map [0, 1] to bin index [0, 63]
                    let binIndex = min(numBins - 1, Int(val * Float(numBins - 1) + 0.5))
                    histogram[binIndex] += 1
                }

                // Step 3: Clip histogram to limit contrast enhancement (prevents noise amplification)
                let clipValue = Int(Float(tile.count) / Float(numBins) * clipLimit)
                var excessPixels = 0

                for i in 0..<numBins {
                    if histogram[i] > clipValue {
                        excessPixels += histogram[i] - clipValue
                        histogram[i] = clipValue
                    }
                }

                // Step 4: Redistribute excess pixels evenly across all bins
                let redistributePerBin = excessPixels / numBins
                let extraPixels = excessPixels % numBins
                for i in 0..<numBins {
                    histogram[i] += redistributePerBin
                    if i < extraPixels {
                        histogram[i] += 1  // Distribute remainder
                    }
                }

                // Step 5: Build cumulative distribution function (CDF)
                var cdf = [Int](repeating: 0, count: numBins)
                cdf[0] = histogram[0]
                for i in 1..<numBins {
                    cdf[i] = cdf[i-1] + histogram[i]
                }

                // Step 6: Apply histogram equalization using CDF
                let cdfMin = cdf.first(where: { $0 > 0 }) ?? 0
                let cdfRange = Float(tile.count - cdfMin)

                guard cdfRange > 0 else { continue }

                // Map each pixel through the equalized CDF
                for y in 0..<tileSize {
                    for x in 0..<tileSize {
                        let globalX = tileX * tileSize + x
                        let globalY = tileY * tileSize + y

                        guard globalY < size && globalX < size else { continue }

                        let idx = globalY * size + globalX
                        let val = floats[idx]
                        let binIndex = min(numBins - 1, Int(val * Float(numBins - 1) + 0.5))

                        // Histogram equalization formula: (CDF[v] - CDF_min) / (size - CDF_min)
                        let equalizedValue = Float(cdf[binIndex] - cdfMin) / cdfRange
                        output[idx] = max(0.0, min(1.0, equalizedValue))
                    }
                }
            }
        }

        return output
    }

}
