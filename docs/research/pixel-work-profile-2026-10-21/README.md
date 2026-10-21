# OrderedPixel budget-category profile

Pinned VectorApple.ai page 0 completed with renderer exit 1. All four sampled attempts still returned incomplete after spending 100,000,000 work units. This is budget attribution, not CPU timing or a performance claim. The instrumented binary hash is in decoder.json; original temporary patches and full stderr are retained. Production sources were restored before running the frozen binary.

Categories are exclusive, nested scopes: setup includes initial clipping; old covers traversal and residual paint bookkeeping; priority includes dominance and source-UV clipping; subtract covers geometric subtraction excluding nested keep/line operations; keep includes retained-piece handling and convex merging; lines covers line interning. Other includes kernel visits. The remaining sampler work includes scans, sorting and work outside OrderedPixel. Category counters increment only after successful debits. Unmetered fixed-size computations are not measured.

| Patches | Setup | Old | Priority | Subtract | Keep | Lines | Other | Sampler remainder |
|---|---|---|---|---|---|---|---|---|
| 1455 | 3833657 | 19003631 | 4145486 | 46607412 | 20965070 | 1388281 | 63675 | 3992788 |
| 323 | 12592158 | 9454551 | 5280963 | 54568130 | 9803239 | 4163466 | 337732 | 3799761 |
| 245 | 4175752 | 22257015 | 4233580 | 41286923 | 23083140 | 1428414 | 70835 | 3464341 |
| 788 | 6754270 | 31087521 | 2589113 | 21109051 | 31455659 | 2200031 | 131670 | 4672685 |

Subtraction accounts for 41–55% of the shared quota in the first three attempts; traversal/retention account for most of the fourth. This evidence directs the next experiment at exact redundant unit-domain clipping, without increasing quotas. AI correctness, the full 100-format scope, pressure testing and hardware qualification remain incomplete.
