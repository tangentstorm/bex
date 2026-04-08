# BDD Optimization Ideas

Based on callgrind profiling of the `tiny` benchmark (factor 210 into 4-bit × 8-bit via BDD).

## Baseline
- **~40-44s** per iteration (cargo bench -- small)
- Callgrind profile top costs: malloc/free ~11%, DashMap ~8%, ITE::norm ~9%

## Untested Ideas

### ~~1. Single-threaded BDD solver for small problems~~ → REJECTED
Optimizes the wrong thing — the real workload (`small`) benefits from threads.

### 2. Replace HiLoCache Mutex with RwLock
`VhlBase::get_hilo()` takes a Mutex lock for every read. Since reads vastly
outnumber writes, an RwLock would allow concurrent readers.

### 3. Pre-size DashMap cache with estimated capacity
The DashMap for job caching (`WorkState::cache`) starts empty and rehashes
repeatedly. Pre-sizing based on expected node count avoids rehashing (1.7% in
hashbrown reserve_rehash).

### 4. Use mimalloc or jemalloc as the global allocator
malloc/free is ~11% of total runtime. A purpose-built allocator like mimalloc
has better multi-threaded allocation patterns and less fragmentation.

### 5. Replace AST HashMap with FxHashMap
`RawASTBase::hash` uses the default SipHash hasher. FxHash is much faster for
small keys (Ops contains a small Vec<NID>).

### 6. Use SmallVec for Wip dependency tracking
`Wip::deps: Vec<Dep>` allocates on the heap. Most nodes have only 1-2 deps.
SmallVec<[Dep; 2]> avoids allocation for the common case.

### 7. Pre-size HiLoCache index
The FxHashMap inside HiLoCache starts small and rehashes. Pre-sizing based on
expected node count avoids this.

### 8. Avoid Vec allocation in Ops
`Ops::RPN(Vec<NID>)` allocates a Vec for every AST node. Since most operations
have 2-3 operands, use a SmallVec or inline array.

### 9. Cache VID comparisons in ITE::norm
`vid.cmp_depth()` is called repeatedly during normalization. VID uses enum
matching; converting to a numeric key for comparison would be faster.

### 10. Reduce channel overhead in swarm
Crossbeam channels are ~1.2% of runtime. For small problems, batch operations
or use a simpler notification mechanism.

### 11. Pre-allocate BDD node storage (boxcar::Vec initial capacity)
boxcar::Vec allocates in chunks. Starting with a larger initial allocation
reduces the number of allocation events.

### 12. Thread-local HiLo read cache
Cache recent HiLo lookups in thread-local storage to avoid Mutex contention
on VhlBase for repeated lookups of the same node.

### 13. Inline VID into a u32 numeric type
VID is an enum (T, NoV, Var(u32), Vir(u32)). Converting to a packed integer
representation would make comparisons a single integer compare instead of
match arms.

### 14. Replace boxcar::Vec with plain Vec behind RwLock
boxcar::Vec has atomic overhead per access. For the tiny benchmark, a plain
Vec with RwLock might be faster since contention is low.

### 15. Avoid re-hashing NormIteKey
NormIteKey wraps ITE which contains 3 NIDs (3×u64). Pre-compute and cache the
hash to avoid re-hashing on every DashMap lookup.

### 16. Use entry API in HiLoCache to combine lookup+insert
Currently get_node() and insert() are separate locked operations. Combining
them into a single lock acquisition halves the lock overhead.

### 17. Reduce Vec grow events in work_job
`raw_vec::grow_one` appears in the profile (0.60%). Pre-allocate Vecs used in
the hot path.

### 18. Fast-path common ITE patterns before full normalization
Many ITE calls resolve to simple cases (both branches equal, one branch
constant). Check these before the full norm() machinery.

### 19. Skip swarm init/teardown per benchmark iteration
BddBase::new() spawns threads each time. Reusing a thread pool across
iterations would eliminate thread spawn overhead.

### 20. Replace DashMap with a simpler concurrent cache
For the tiny benchmark, DashMap's sharding overhead may not be worth it.
A simple Mutex<FxHashMap> or even single-threaded HashMap could be faster
when there's little parallelism to exploit.

### 21. Eliminate Ops::norm() allocations
Ops::norm() creates a new Vec every time. For hot-path normalization, operate
on a fixed-size array or modify in place.

### 22. Use swap solver's direct approach for BDD
The swap solver (SwapSolver) is noted as 2x faster than BDD. Analyze what
makes it faster and apply those principles to the BDD solver.

### 23. Lazy thread spawning in swarm
Don't spawn worker threads until actually needed. For small problems,
the main thread might complete before workers even start.

### 24. Reduce solve() overhead - batch AST node processing
Each refine_one() call processes one AST node. Batching multiple
substitutions could reduce per-step overhead.

### 25. Remove or simplify ITE::norm()
ITE::norm() is 4.5% of runtime plus significant inlined cost in nid.rs/vid.rs.
It performs complex normalization with many branches. Test whether removing or
simplifying the normalization (just using a canonical ordering) actually helps
or hurts — it may create more cache misses but save normalization cost.
(Suggested by project owner — "always taken on faith that it's an optimization")

## Optimizations Applied

### 2 + 16. RwLock for HiLoCache + combined get_or_insert
Replaced Mutex with RwLock on HiLoCache (reads vastly outnumber writes).
Added `get_or_insert` method that tries a read lock first, only upgrading
to write lock on cache miss. Combined these avoid double-locking in
`vhl_to_nid`. ~7% improvement (34s avg vs 37s baseline).

### DashMap: increase shard count from 16 to 128 + pre-size to 16K entries
The default 16 shards meant high contention per shard with 40M operations.
128 shards dramatically reduces lock contention. Pre-sizing avoids early
rehashing. ~14% improvement over previous (30s avg vs 34s). Cumulative
~19% improvement vs original baseline (30s vs 37s).

## Rejected Ideas

### 1. Single-threaded BDD solver
Wrong target — `small` benefits from multiple threads.

### 25. Remove/simplify ITE::norm()
Tested two variants: (a) removed cmp!-based rewriting rules, (b) removed all
canonicalization except constant folding. Both showed no speedup — in fact
slightly worse due to more cache misses (40M tests vs 39.8M). The normalization
IS earning its keep through better cache hit rates.

### 4. mimalloc allocator
No clear improvement (within noise). The allocation overhead is from the
sheer number of allocations, not allocator inefficiency.

### 3. Pre-size DashMap / SmallVec for deps and jobs
No clear improvement. DashMap pre-sizing doesn't help because the bottleneck
is per-operation cost, not rehashing. SmallVec for deps made entries larger,
potentially hurting cache performance.
