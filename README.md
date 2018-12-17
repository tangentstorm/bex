# bex
A rust library for working with boolean expressions (expression trees, decision diagrams, etc.)

This crate lets you build a complicated abstract syntax tree (or logic circuit schematic, if you prefer) by working with individual Bit structs, or vectors that act like integers. You can also solve these AST structures by converting them into reduced, ordered, binary decision diagrams (ROBDDs) - a normal form consisting of if-then-else triples that essentially act like compressed truth tables. You can also construct and manipulate BDDs directly.
