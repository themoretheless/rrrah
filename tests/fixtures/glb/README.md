# Original Khronos Box GLB fixture

Box.glb is the unmodified Cesium Box sample, copyright 2017 Cesium,
licensed under [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/legalcode).
Author: Cesium. No changes were made to the sample bytes.

Source: https://github.com/KhronosGroup/glTF-Sample-Assets/tree/8f492390ddc7a2ae221a1369ef560589cf8c70fb/Models/Box

The source model README is retained as upstream-README.md. `manifest.json`
records byte framing independently read with Python struct, not the native parser.
The current native checkpoint verifies container/source ownership only; scene,
geometry, material, cache/swap and GPU validation are still required.
