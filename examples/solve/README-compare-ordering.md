# Compare Ordering Tool

This tool compares BDD sizes between bex's default variable ordering (x0 at bottom) and traditional BDD ordering (v0 at top).

## Background

Most BDD systems order variables with x0 at the top of the diagram. Bex differs by placing x0 at the bottom. This tool allows testing the hypothesis that bex's approach leads to smaller BDD bases for certain classes of functions.

## The `tradord` Feature

The `tradord` compilation feature changes how virtual variables (virs) are ordered:

- **Default behavior**: virs are ordered like vars, with v0 below v1, below v2, etc. All virs are above all vars.
- **With `tradord` enabled**: virs are ordered in the traditional way, with v0 at the very top (above v1, v2, etc.)

This feature only affects vir ordering, not var ordering. Vars always maintain bex's ordering (x0 at bottom).

## Usage

### Building

```bash
# Build without tradord (default)
cargo build --bin compare-ordering

# Build with tradord feature enabled
cargo build --bin compare-ordering --features tradord
```

### Running

```bash
# Run with test data
cargo run --bin compare-ordering test-data/*.tt

# Run with tradord feature
cargo run --bin compare-ordering --features tradord test-data/*.tt
```

### Input Format

Truth table files should contain binary data where each byte is either 0 or 1, representing the output of a Boolean function for each input combination. The file size must be a power of 2 (2^n for n variables).

For example, a 4-byte file represents a function of 2 variables:
- Byte 0: f(0,0)
- Byte 1: f(1,0)
- Byte 2: f(0,1)
- Byte 3: f(1,1)

## Creating Test Data

The `test-data` directory contains several example truth tables created with Python:

```python
# XOR function (2 variables)
xor = [0, 1, 1, 0]
with open('xor-2vars.tt', 'wb') as f:
    f.write(bytes(xor))

# Multiplexer: x0 ? x2 : x3 (4 variables)
mux = []
for i in range(16):
    x0, x1, x2, x3 = (i>>0)&1, (i>>1)&1, (i>>2)&1, (i>>3)&1
    mux.append(x2 if x0 else x3)
with open('mux-4vars.tt', 'wb') as f:
    f.write(bytes(mux))
```

## Implementation Details

The tool:
1. Reads truth table files
2. Builds a BDD using vars (x0, x1, ...) - bex's natural ordering
3. Builds a BDD using virs (v0, v1, ...) - which follows tradord when enabled
4. Counts nodes in each BDD and compares sizes

Variable ordering affects BDD size significantly. Functions with structure that aligns with the ordering tend to produce smaller BDDs. This tool helps quantify the difference between orderings for different function classes.

## Results

Initial testing shows that:
- Symmetric functions (XOR, parity) have similar sizes regardless of ordering
- Functions with locality (like multiplexers) can show significant differences
- The mux-4vars example showed 25% smaller BDD with bex's ordering

## Future Work

- Test with larger, more complex functions
- Analyze classes of functions that benefit from each ordering
- Extend to support variable reordering algorithms
