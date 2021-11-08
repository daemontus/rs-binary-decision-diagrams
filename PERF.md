# Performance investigation

Everything is happening in the `perf_testing.rs` module, so that it can be simply copied into compiler explorer and analyzed.

## Core design

A BDD node assumes a 32-bit variable and 48-bit addresses, so that it can be packed into 128 bits. Technically, the address bits are split into the upper bits of two 64-bit values. This has the disadvantage that unpacking values takes a bit of bit-manipulation, but it is generally reasonable because it saves us memory that we would generally waste and that would pollute caches. Meanwhile, unpacking is generally performed once and then the results are cached in registers or buffers.

## Testing

Each algorithm/question has a separate markdown file with test data.

 - `PERF_TRAVERSAL.md` Contains findings about performance of a basic DFS search.