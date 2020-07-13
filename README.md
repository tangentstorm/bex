# bex

A rust library for working with boolean expressions
(expression trees, decision diagrams, etc.)

This crate lets you build a complicated abstract syntax tree (or logic
circuit schematic, if you prefer) by working with individual Bit
structs, or vectors that act like integers. You can also solve these
AST structures by converting them into reduced, ordered, binary
decision diagrams (ROBDDs) - a normal form consisting of if-then-else
triples that essentially act like compressed truth tables. You can
also construct and manipulate BDDs directly.

## Changes in 0.1.4

This version introduces the ANFBase for working with algebraic normal
form using a BDD-like graph structure. This version also introduces
Cursors, which provide the ability to iterate through BDD solutions
and ANF terms.

It also includes a major refactoring effort: the BDD, AST, and ANF
bases now all use the same NID/VID types for node and variable
identifiers.

Finally, BDD and ANF graphs are now arranged so that variables with
the smallest identifiers now appear at the bottom (so that subgraphs
are more likely to be shared across functions with different numbers
of inputs, and also so that the size of a node's truth table is
immediately apparent from its topmost variable.)

For more detail on these and other changes, see [CHANGELOG.md](https://github.com/tangentstorm/bex/blob/main/CHANGELOG.md).
