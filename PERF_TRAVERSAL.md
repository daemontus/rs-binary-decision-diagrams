
## BDD traversal

First thing that we have to measure is how fast a BDD can be traversed depending on its in-memory representation. Here are the numbers for a relatively naive implementation (commit `878bfa78b525148a40ca5eea112cd46ba41bd479`) with an unsorted BDD:

| Benchmark | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Node | C/Node |
| --------- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 12239682 | 351912898 | 612306262 | 11372357 | 3011874 | 1.74 | 73.52 | 50.03 | 28.75 |
| large-same-larger.12779394 | 12779394 | 355725516 | 626789183 | 11417000 | 3162234 | 1.76 | 72.30 | 49.05 | 27.84 |
| large-same-larger.96221488 | 96221488 | 3026780386 | 4841501049 | 96020984 | 25475735 | 1.60 | 73.47 | 50.32 | 31.46 |
| large-same-same.176900752 | 176900752 | 5443417235 | 8901077176 | 182407543 | 47489443 | 1.64 | 73.97 | 50.32 | 30.77 |

Meanwhile, having the BDD sorted in strict post-order cripples the whole algorithm:

| Benchmark | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Node | C/Node |
| --------- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 12239682 | 510090186 | 612574973 | 8787613 | 3110733 | 1.20 | 64.60 | 50.05 | 41.68 |
| large-same-larger.12779394 | 12779394 | 523644298 | 627039574 | 7987167 | 2843132 | 1.20 | 64.40 | 49.07 | 40.98 |
| large-same-larger.96221488 | 96221488 | 4519212891 | 4847252866 | 57857404 | 18115875 | 1.07 | 68.69 | 50.38 | 46.97 |
| large-same-same.176900752 | 176900752 | 8369474334 | 8902257292 | 108996237 | 33723060 | 1.06 | 69.06 | 50.32 | 47.31 |

And sorting based on pre-order makes it significantly faster:

| Benchmark | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Node | C/Node |
| --------- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 12239682 | 233352072 | 612251626 | 8095895 | 461117 | 2.62 | 94.30 | 50.02 | 19.07 |
| large-same-larger.12779394 | 12779394 | 232320849 | 626781904 | 7631249 | 239176 | 2.70 | 96.87 | 49.05 | 18.18 |
| large-same-larger.96221488 | 96221488 | 1939904463 | 4841298479 | 65164027 | 4272823 | 2.50 | 93.44 | 50.31 | 20.16 |
| large-same-same.176900752 | 176900752 | 3546036107 | 8900694113 | 120964747 | 7915088 | 2.51 | 93.46 | 50.31 | 20.05 |

Finally, by using unsafe code that avoids bounds checking, we can eliminate ~20 instructions per iteration and get the following final result (commit `f2b32293f05acc7481d4ba8a9d4c3fa418cba6df`):

| Benchmark | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Node | C/Node |
| --------- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 12239682 | 194212656 | 355208717 | 8337984 | 703814 | 1.83 | 91.56 | 29.02 | 15.87 |
| large-same-larger.12779394 | 12779394 | 189368964 | 358389153 | 7708219 | 359218 | 1.89 | 95.34 | 28.04 | 14.82 |
| large-same-larger.96221488 | 96221488 | 1519381690 | 2820479262 | 66467270 | 5588365 | 1.86 | 91.59 | 29.31 | 15.79 |
| large-same-same.176900752 | 176900752 | 2727206396 | 5190562772 | 122506143 | 9439052 | 1.90 | 92.30 | 29.34 | 15.42 |

Note that strictly speaking, this version has worse IPC and hit rates, but it still performs ~4-5 cycles per node less than the basic version with a sorted BDD.