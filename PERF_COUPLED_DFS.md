## Coupled DFS Performance

Coupled DFS explores the `MxN` product of two BDDs starting in their respective roots. This is more complicated than a naive DFS, because we actually need a hash set to save visited nodes, since there can be up to `MxN` nodes, but usually the number is much smaller. So keep in mind that the nodes in the following tables are "virtual" nodes in the produce graph. At the same time, this is the "complexity measure" that determines the actual runtime of the `apply` algorithm. 

Here are data for a naive version (commit `5e99edba7cbc226301432510f1aa52c7ce49b56c`) and post-order BDDs that uses FxHash (default hash is 3-4x worse).

| Benchmark | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Node | C/Node |
| --------- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 17318883 | 6368756230 | 5081731959 | 205293543 | 72501808 | 0.80 | 64.68 | 293.42 | 367.73 |
| large-same-larger.12779394 | 22786148 | 7867883525 | 6238077000 | 246534026 | 93614123 | 0.79 | 62.03 | 273.77 | 345.29 |
| large-same-larger.96221488 | 176923014 | 102813256219 | 50761899406 | 2341582664 | 896521471 | 0.49 | 61.71 | 286.92 | 581.12 |
| large-same-same.176900752 | 176957574 | 103490910627 | 41530326694 | 2047876353 | 910090927 | 0.40 | 55.56 | 234.69 | 584.83 |


In terms of instructions, this is almost 10x worse than basic traversal, but more worryingly, we see that the IPC is worse for larger benchmarks, resulting in larger cycle counts. The L3 hit rate is also falling along with IPC, because the usage of the hash map kills any memory locality. Note that sorting the BDDs by preorder does not have a meaningful impact on this algorithm because majority of the latency comes from the hash map.

We can replace the default hash set with a "leaky" hash set (commit `a48ec15ffeda5bbb0e6fa1d374adc7d45a083d14`) that will override on collision. This introduces a small number of redundantly computed tasks due to collisions, but slashes the instruction and cycle count roughly by half.

| Benchmark | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Node | C/Node |
| --------- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 17338303 | 3436204933 | 2102466984 | 91124736 | 39728640 | 0.61 | 56.40 | 121.26 | 198.19 |
| large-same-larger.12779394 | 22800201 | 4543658877 | 2701547055 | 106987193 | 44909204 | 0.59 | 58.02 | 118.49 | 199.28 |
| large-same-larger.96221488 | 190219394 | 41308733615 | 22233715981 | 1560979203 | 1023407216 | 0.54 | 34.44 | 116.88 | 217.16 |
| large-same-same.176900752 | 177128830 | 37848725739 | 22209141668 | 1075268071 | 394285562 | 0.59 | 63.33 | 125.38 | 213.68 |

However, in terms of IPC and L3 hit rates, this is still terrible. The BDD node ordering is starting to matter a bit (about 5-10%), but nowhere near what we have seen for basic traversal (yet).

If we replace Knuth-hashing with a more locality based algorithm, we mostly get a significant improvement in IPC and L3 hit rate (commit `cc5c4c88653cb7f51065610a4ea59e7e4e5085bd`), rven though the number of collisions increases. We still get a significant improvement even in postorder:

| Benchmark | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Node | C/Node |
| --------- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 18319005 | 1904099613 | 2415165490 | 85916236 | 34115215 | 1.27 | 60.29 | 131.84 | 103.94 |
| large-same-larger.12779394 | 25220517 | 2407991830 | 3249919944 | 102768751 | 40297614 | 1.35 | 60.79 | 128.86 | 95.48 |
| large-same-larger.96221488 | 182802890 | 19977114037 | 23428538677 | 679835980 | 243329016 | 1.17 | 64.21 | 128.16 | 109.28 |
| large-same-same.176900752 | 369149571 | 40238932306 | 47008334888 | 1198451197 | 338426577 | 1.17 | 71.76 | 127.34 | 109.00 |

In preorder, the numbers are even better (collisions also improved for some reason, but they are still problematic):

| Benchmark | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Node | C/Node |
| --------- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 18318177 | 1121256266 | 2414864526 | 80606493 | 30961959 | 2.15 | 61.59 | 131.83 | 61.21 |
| large-same-larger.12779394 | 24330050 | 1411447401 | 3142423419 | 101888646 | 40782037 | 2.23 | 59.97 | 129.16 | 58.01 |
| large-same-larger.96221488 | 182467686 | 11611632730 | 23385806302 | 930560799 | 346602007 | 2.01 | 62.75 | 128.16 | 63.64 |
| large-same-same.176900752 | 249493645 | 21687786248 | 32764755006 | 916299797 | 213111617 | 1.51 | 76.74 | 131.33 | 86.93 |

Note that here, we are at 4-6x the cycle count of a basic traversal algorithm (not accounting for collisions). The question is how much further can we push this.

If we eliminate bounds checking where possible, add a little prefetching and optimise instructions (including actually hashing just half the node pair), we can get to almost 2-3x the cycle count of the basic traversal algorithm (commit `d684964ef215fd83612772a7e20fd97981ced265`):

| Benchmark | Nodes | Cycles | Instructions | L3 References | L3 Misses | IPC | L3 hit | I/Node | C/Node |
| --------- | ----- | ------ | ------------ | ------------- | --------- | --- | ------ | ------ | ------ |
| large-same-same.12239682 | 18498017 | 604242373 | 1516224747 | 58050756 | 9205795 | 2.51 | 84.14 | 81.97 | 32.67 |
| large-same-larger.12779394 | 24516570 | 751023135 | 1998083303 | 68937696 | 10800220 | 2.66 | 84.33 | 81.50 | 30.63 |
| large-same-larger.96221488 | 182610133 | 5815784497 | 14956392931 | 551605989 | 64741676 | 2.57 | 88.26 | 81.90 | 31.85 |
| large-same-same.176900752 | 180363858 | 7137708151 | 16113311498 | 912219354 | 104313099 | 2.26 | 88.56 | 89.34 | 39.57 |

We still have some IPC reserves, but our L3 hit rate is quite good and overall this is not too bad (considering we are comparing with a super basic traversal). Also, note that the collision rate in the final algorithm is 3-7%, which is all right. However, keep in mind that on a truly representative sample inputs, we would have to track this and expand the cache accordingly. 

On older CPUs (Zen1), the progression is a bit less impressive in absolute numbers, but we are still talking about going from 580-920 C/Node and 0.6-0.3 IPC to 55-65 C/Node and 1.2-1.3 IPC. But the cache hit rates look good, so I'm not sure what is going on there, maybe just an older core with larger instruction latencies. Dependencies in the loop body are an obvious problem, but I'm not going to mess with that since they are not going to matter for the actual `apply` algorithm. 