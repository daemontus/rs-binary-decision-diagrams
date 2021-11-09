## Apply Algorithm Performance

First, we start with a relatively simple implementation using native `HashMap` with FxHash and unordered BDDs (commit `9d84e8e090aa9459f5941bcb9c598bb20deb4b11`):

| Benchmark | Tasks | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Task | C/Task |
| --------- | ----- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 17318880 | 12239680 | 12516622994 | 8574655122 | 423932320 | 158843416 | 0.69 | 62.53 | 495.10 | 722.72 |
| large-same-larger.12779394 | 22786145 | 17347648 | 18192610887 | 12478321950 | 642806833 | 215790336 | 0.69 | 66.43 | 547.63 | 798.41 |
| large-same-larger.96221488 | 176923011 | 176899022 | 197159171642 | 103436568829 | 5807427224 | 2206608280 | 0.52 | 62.00 | 584.64 | 1114.38 |
| large-same-same.176900752 | 176957571 | 176900750 | 185689451538 | 82462294650 | 4831738215 | 2203440028 | 0.44 | 54.40 | 466.00 | 1049.34 |

The performance is in line with what we've seen from older implementations. Interestingly, it isn't that far off from the coupled DFS search. A 10x improvement in C/Task would be certainly welcome here, but probably a bit unrealistic (anything under 150-200 would be a win here). We also see a decreasing IPC trend, but it is not as pronounced due to the higher instruction count.

Also note that we now call "Nodes" the number of newly created nodes, and "Tasks" the number of explored "virtual nodes" in the `MxN` graph.

A logical first step is to switch to pre-ordered BDDs and replace the generic task cache with a lossy locality-preferential cache (commit `de2223687e76aa16fb44e5a414c5ce90a66a6c8a`):

| Benchmark | Tasks | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Task | C/Task |
| --------- | ----- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 18488274 | 12239680 | 6934664067 | 5939768656 | 248523419 | 94378137 | 0.86 | 62.02 | 321.27 | 375.08 |
| large-same-larger.12779394 | 24508559 | 17347648 | 11198860838 | 9503619903 | 428861114 | 133248135 | 0.85 | 68.93 | 387.77 | 456.94 |
| large-same-larger.96221488 | 182453848 | 176899022 | 134512902396 | 75671660419 | 3859308368 | 1325104444 | 0.56 | 65.66 | 414.74 | 737.24 |
| large-same-same.176900752 | 180335595 | 176900750 | 132948522319 | 66726824283 | 3733980915 | 1507209100 | 0.50 | 59.64 | 370.01 | 737.23 |

This is a sizeable improvement, but still quite far from anything that we are aiming for. We can save a few percent by using a stack without bounds checking, but this is also relatively useless at this point. Another small improvement comes from caching task variables, as these require two "random" accesses to recover once task is finished and thus makes sense to save them on the stack instead. This is better than stack optimization because it eliminates memory access, but is still within 10-15% of the figures above.

However, the important step is replacing the node storage with locality-based cache. In such case, we have again a drop in instruction count and a massive increase in IPC (commit `b310ce20c223886bf041e5837200a81dd0b57e70`):

| Benchmark | Tasks | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Task | C/Task |
| --------- | ----- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 18488274 | 12239682 | 2746906333 | 4739196485 | 154303369 | 34031077 | 1.73 | 77.95 | 256.34 | 148.58 |
| large-same-larger.12779394 | 24508559 | 17347650 | 4057196148 | 6307995222 | 205674575 | 46427889 | 1.55 | 77.43 | 257.38 | 165.54 |
| large-same-larger.96221488 | 182453848 | 176899024 | 37746272222 | 48976649419 | 1810963915 | 409317326 | 1.30 | 77.40 | 268.43 | 206.88 |
| large-same-same.176900752 | 180335595 | 176900752 | 32573735734 | 49637965869 | 2149082703 | 474415197 | 1.52 | 77.92 | 275.25 | 180.63 |

This is still much more than pure coupled DFS, but at least we are performing somewhere within 5-7x the coupled DFS algorithm. Now the important question is how big of a bottleneck is the node cache. If we replace it with an implementation that does not maintain uniqueness, just computes the hash and saves the node in the first available slot, then our C/Task drops to 120-150 and IPC goes is around 1.7-2.0. This is without significant reduction in instruction count, so the effect is almost purely due to latency and branch prediction.

The question is, how much of this can be "saved" by going fully out-of-order. Because the number of instructions will certainly be larger, so the question is how much of it can be absorbed by improvements in IPC. By running two instances of the same algorithm concurrently, the answer appears to be "not much", but hopefully there is a way around that.

Also, interestingly, rewriting the algorithm to use stack more "natively" yields worse performance and higher instruction count (see `9b0dd7f5f3a73c5d034bc29eaf9f3605d3afdcbd`). Allocating a single big "task object" actually appears to be quite efficient. However, one way to save a few instructions is to realize that the task cache can be modified such that it does not need modulo. This avoids a few divisions. Finally, various attempts at loop unrolling also seem to do more harm than good.

So far, the best in-order design that we have is capable of this:

| Benchmark | Tasks | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Task | C/Task |
| --------- | ----- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 18488252 | 12239682 | 2438729911 | 4141600774 | 151919123 | 34253922 | 1.70 | 77.45 | 224.01 | 131.91 |
| large-same-larger.12779394 | 24508505 | 17347650 | 3712414971 | 5508428188 | 206362767 | 49256678 | 1.48 | 76.13 | 224.76 | 151.47 |
| large-same-larger.96221488 | 182453847 | 176899024 | 34970435426 | 42998464443 | 1805149277 | 420372874 | 1.23 | 76.71 | 235.67 | 191.67 |
| large-same-same.176900752 | 180335595 | 176900752 | 30368388034 | 43872237559 | 2177768670 | 490504172 | 1.44 | 77.48 | 243.28 | 168.40 |
