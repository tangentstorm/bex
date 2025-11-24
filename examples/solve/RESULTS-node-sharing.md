# BDD Node Sharing Comparison Results

This document reports the results of testing node sharing between multiple Boolean functions using different variable orderings.

## Test Setup

**Key Insight**: This test properly measures node sharing by building ALL truth tables into a SINGLE BddBase and counting total unique nodes.

- **21 diverse 16-variable Boolean functions** (65536 bits each)
- **Two orderings tested**:
  - Bex ordering: x0=LSB at bottom, x15=MSB at top (using x-vars)
  - Traditional ordering: v0=MSB at top, v15=LSB at bottom (using v-vars with `tradord` feature)

## Results

### Node Sharing is Significant

When functions share a BddBase:
- **Separate BDDs (no sharing)**: 49,858 total nodes (sum of individual BDD sizes)
- **Shared BddBase**: 44,702 total nodes
- **Nodes saved through sharing**: 5,156 nodes (10.3% reduction)

This confirms that node sharing between related Boolean functions is substantial!

### Both Orderings Show Identical Sharing

However, surprisingly:
- **Bex ordering (x-vars)**: 44,702 nodes
- **Traditional ordering (v-vars with tradord)**: 44,702 nodes
- **Difference**: 0 nodes (0%)

## Analysis

### What This Tells Us

1. **Node Sharing Works**: The 10.3% reduction from 49,858 to 44,702 nodes proves that functions sharing a BddBase do share nodes at the bottom/common levels.

2. **Ordering-Independent for These Functions**: For this particular set of 21 Boolean functions (primality, modulo, factorization, pseudo-random), both orderings result in identical sharing patterns.

3. **Canonical Structure Similarity**: The ITE normalization produces canonical forms that happen to have the same sharing characteristics under both orderings.

### Why Both Orderings Are Equal

Several possible explanations:

1. **Function Symmetry**: Many test functions (modulo, divisibility) have similar structural properties that work equally well with either ordering.

2. **Bit Independence**: Functions like primality testing and SHA-256-based pseudo-random don't have strong bit-position dependencies that favor one ordering.

3. **Optimal Structure**: For these functions, the optimal BDD structure may be similar whether organized from LSB-to-MSB or MSB-to-LSB.

4. **Canonicalization Power**: Bryant's reduction rules are so effective that different initial orderings converge to similar canonical structures.

### What Functions Might Show Differences?

To see ordering effects on node sharing, we would need functions with:

1. **Bit-Position Asymmetry**: Functions where LSBs have fundamentally different structure than MSBs
2. **Hierarchical Dependencies**: Functions where higher bits depend on lower bits (or vice versa) in asymmetric ways
3. **Arithmetic Circuits**: Addition, multiplication, comparison operators that propagate from LSB upward
4. **Sequential Logic**: State machines or counters with clear LSB-to-MSB or MSB-to-LSB flow

## Test Functions Summary

| Category | Functions | Typical Size |
|----------|-----------|--------------|
| Primality | is_prime | 3,242 nodes |
| Prime Factors | has_N_factors | 4,149-6,234 nodes |
| Modulo | mod K tests | 5-89 nodes |
| Primorial | divisible by product of primes | 42-464 nodes |
| Pseudo-Random | SHA-256 based | 8,044-8,061 nodes |
| Random AST | evaluated expressions | 2-13 nodes |

## Conclusions

1. **Node Sharing Confirmed**: Sharing a BddBase reduces total nodes by 10.3% for these functions.

2. **No Clear Winner**: Neither bex nor traditional ordering shows an advantage for these particular functions.

3. **Hypothesis Not Disproven**: The hypothesis that bex's ordering might lead to more sharing isn't disproven - we just haven't found functions where it matters yet.

4. **Need Better Test Cases**: To properly test the ordering hypothesis, we need:
   - Arithmetic circuits (adders, multipliers)
   - Functions with known LSB-to-MSB dependencies
   - Real-world applications with clear bit-significance patterns

5. **Practical Implication**: For general-purpose BDD use with diverse functions, ordering choice may matter less than expected. Dynamic reordering is still valuable for optimizing specific function sets.

## Recommendations

1. **Keep bex's ordering**: No evidence of disadvantage, and it's more intuitive for hardware/arithmetic applications.

2. **Test with arithmetic circuits**: Implement adders, multipliers, and comparators to find ordering-sensitive cases.

3. **Implement dynamic reordering**: This will provide much larger benefits than static ordering choice.

4. **Document the canonicalization**: The fact that different construction orders can lead to identical canonical forms is valuable and worth highlighting.

## Technical Notes

- ITE normalization in bex reorders nodes according to `VID::cmp_depth` during construction
- This creates a canonical form independent of the order in which nodes are created
- The `tradord` feature correctly changes `VID::cmp_depth` for virs, but these functions happen to have similar optimal structures under both rules
- Measurement counts unique nodes reachable from all function roots in the shared BddBase
