
# bex examples

**NOTE:** these examples are kind of a mess at the moment. I'm documenting them as-is partially to make the cleanup process a bit easier next time I work on this.

## bdd-subst

This is my main benchmark:

    # run from top level directory
    $ cargo run --bin bdd-solve

**NOTE**: this might take years to run. get ready to press ^C.

Smaller benchmarks can be run with:

    $ rustup install nightly  # if you haven't already.

	$ cargo +nightly bench --bin bdd-solve bench_tiny
	$ cargo +nightly bench --bin bdd-solve bench_small

Even the "small" benchmark is currently painfully slow, which is sad, because it's just solving to find two u8 values that multiply together to get the u16 value 210... Something you can probably do faster with a pencil and paper.

The 'bench' commands of course run the same thing over and over many times. There are also 'test' commands, but these currently require `graphviz` and `firefox` to be installed and on your path.

(I know this is dumb: tests shouldn't have side effects. I intend to clean this up. Feel free to comment the relevant lines out, or just install graphviz from http://graphviz.org/ )


## bex-shell

This is a rudimentary shell that lets you build up expressions interactively.

It's *extremely* rough and probably not terribly useful right now. (It only
uses AST expressions... At one point it was using BDD nodes, but this is commented out until I get around to unifying base::TBase with bdd::Base.

    # from top-level directory:
    cargo run --bin bex-shell

The syntax is forth-like, meaning each whitespace delimited token is executed in sequence from left to right, and each word takes and consumes a stack of values. The values are generally treated as NIDs.

The words are:

    <any integer n>  -> push n onto the stack
	i       -> push I (true) onto the stack
    o       -> push O (false) onto the stack
	q       -> quit
	.       -> print and drop topmost item
    dot     -> show the dot syntax for the current node
    sho     -> actually render it (panics if dot/graphviz not installed)
    not     -> negate the node on the stack
    vars    -> allocate <top number on stack> vars
    drop    -> drop topmost item from stack
	dup     -> copy topmost item
	swap    -> swap top two items
    reset   -> clear the stack


example:

    > 4 vars $0 $1 and $2 or
    [ 7 ]
    > dot
    digraph bdd {
    rankdir=BT;
    node[shape=circle];
    edge[style=solid];
      7[label=∨];
     4->7;
     6->7;
    4[label="$2"];
      6[label=∧];
     2->6;
     3->6;
    2[label="$0"];
    3[label="$1"];
    }
    [ ]
