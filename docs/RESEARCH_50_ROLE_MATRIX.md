# 50-role research matrix

Пятьдесят проходов объединены в десять групп по пять ролей. Это не пятьдесят
непроверяемых мнений: каждая роль должна привязать вывод к исходнику, commit,
paper, issue или измеряемому benchmark.

## A. Конкуренты и production workflows

1. darktable architecture; 2. RawTherapee pipeline; 3. RapidRAW/WGPU;
4. Lightroom/ACR workflow; 5. Capture One/FastRawViewer latency.

## B. RAW formats и decoders

6. RawSpeed SIMD; 7. LibRaw API; 8. rawler internals; 9. DNG SDK/opcodes;
10. TIFF/BigTIFF/float RAW.

## C. CPU parallelism

11. CR2 entropy dependencies; 12. restart-marker partitioning; 13. DNG tile
workers; 14. SIMD predictor/linearization; 15. NUMA/affinity/oversubscription.

## D. GPU architecture

16. fragment vs compute; 17. texture arrays/atlas; 18. workgroup/shared memory;
19. subgroup reductions; 20. Metal/Vulkan/DX12 portability.

## E. Memory, storage и cache

21. OS page cache; 22. RAM TinyLFU/2Q; 23. GPU residency; 24. staging rings;
25. persistent tile formats/compression.

## F. Image mathematics

26. Bayer/X-Trans demosaic; 27. MHC/RCD/AMaZE; 28. black/white/linearization;
29. WB/matrix/ICC/DCP; 30. HDR/tone/gamut.

## G. Computational photography

31. burst denoise; 32. HDR bracket merge; 33. panorama alignment/blending;
34. focus stack/super-resolution; 35. learned restoration.

## H. Photo practice и user research

36. pixls.us demosaic practice; 37. darktable issues; 38. RawTherapee forums;
39. RapidRAW issues; 40. photographer acceptance criteria.

## I. Reliability, security, legal

41. malformed RAW/fuzzing; 42. GPU/device loss; 43. deterministic math;
44. model/codec/profile licensing; 45. privacy/metadata/plugin sandbox.

## J. Benchmarking и release

46. statistical methods; 47. quality corpus/oracles; 48. cross-platform CI;
49. live telemetry/dashboard; 50. regression/rollback/release gates.

### Evidence rating

```text
E0 = speculation only
E1 = paper/benchmark prototype
E2 = open-source implementation
E3 = repeated production practice
E4 = independently reproducible in our harness
```

Новые функции не попадают в P0 по одному E1 paper result. Для P0 требуется
минимум E2, numerical oracle и E4 benchmark; для quality-critical develop
модулей — E3/E4.
