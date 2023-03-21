/*


row swapping algorithm  (1/20/2021)
---------------------------------------------------

Swap works on a single pair of rows at once.

It rewrites nodes in place, so that the branch variable of node may change.
(Which is why we use XIDs for reference instead of NIDs.)

0. Definitions and Observations
   w, rw: old top var, and its row
   v, rv: new top var, and its row
   mut gc: Vec<XID> of reclaimed xids.

   n0. Note that v nodes can never initially refer to w nodes.
   n1. Nodes in rw may or may not refer to rv.
   n2. Nodes in rv that have no references outside rw will be garbage collected.


## Constrained Resources: cpu time, ram, xids
  - swapping should be the most expensive task in the swap solver.
    so we don't want to stop work just to wait for an allocation of new xids.
  - but rows may become so large that ram is also a constraint.
    this means we don't want to allocate huge WIP if we can avoid it.
  - assume we are working in isolation from the main thread, perhaps even across
    the network.
  - so: try to keep a queue of fresh xids to use, proportional to the
    size of the row we're swapping.


#. Move nodes on rw with references to rv up to rv:

  We can think of two different groups of nodes on rw:

  - group I (independent):
    These are nodes that do not reference row v.
    These remain on rw, unchanged.

    This group becomes the new rw.

 - group D (dependent):
   These are nodes with at least one child on row v.
   These must be rewritten in place to branch on v (and moved to rv).
   "In place" means that their XIDs must be preserved.
   The moved nodes will have children on row w:
     These may be new nodes, or may already exist in group I.
   The old children (on row v) may see their refcounts drop to 0.

  #. let mut rw' = new row
  #. drain rw -> rw':
    - if group I: add to rw'
    - if group D:
      For each node@(nv,hi,lo): dec_ref(hi) dec_ref(lo) if node.rv == 0

  #. garbage collect rv, including rv nodes whose rc was already 0.
     We are never going to throw away a node that we'll need to recreate
     here, because the only nodes added to rv will move from rw and have new
     children on rw. By observation n0, they must be completely fresh nodes.

  #.
    if node.rc==0 { rw.delete(node);  gc.push(node.xid) }
    else { for child:XID in vec![node.hi, node.lo] {}}









---------------------

Modify the substitution solver:
  init(vid) (just change nid->vid)
  subst(vid, &base, nid)



other ideas (1/17/2021)
------
the starting AST has thousands of nodes. currently, it's a binary tree labeled with boolean operators.

let's mark each AST node with the highest var on which it depends, and add a second bit for whether the
node directly references that var.

I'm picturing something a little like the "World" concept from ometa: a world is a chained dictionary,
much like an iheritance chain.

But now imagine that the AST is stored in a single dictionary of nid->def, and use that as the start of
our world.

Most nodes do not depend directly on vars, but rather on intermediate results (virs).

id->def (for every item in the AST), and then at each step, we introduce a pair of worlds (new dictionaries,
but with lookup chaining to the previous dictionary)

... the idea is a little fuzzy in my head... but what if the worlds themselves could be arranged in
a hi lo scaffold?

let's say we have the AST floating off in space somewhere, and we observe that there are a handful of entries
that depend only on variable 0. well, there should be exactly 1 - 0 itself.

Okay, scratch that: let's say we've defined the AST such that expressions of the bottom 5 inputs are
represented by truth tables.

So now let's say the "basement" of the scaffold (the leaves) are composed of constant NIDS that directly
represent 32-bit truth tables. This row is actually virtual since any references we'd keep to the entries
have enough space to store the entire truth table. These nodes have cost=0.

There are also a bunch of nodes that represent individual input variables, and these have cost=1.
Since we're not talking about a swap solver here, these are encoded directly in the nid and so also
go in the virtual "basement."

So we'll sort the AST by cost.

We can represent each node by its definition:  id -> (fn:id, inputs:vec[id])
Note that the function is expressed in terms of an ID, so AND becomes the level2 const nid with And's truth table.
(0b0001 repeated 8 times to make a 32-bit table, so 0x11111111).

This also means that we have instant access to 2^32 combinators out of context, as well as any number of combinators
we define ourselves by switching from a const nid to a reference to some context.

Now we start to construct the scaffold. Loop through the entries of the cost-ordered AST,
and just move them one at a time over to the scaffold whenever the number of inputs is 0.

We can rewrite a function to have fewer inputs whenever:

  1. one or more of the inputs is constant
  2. two or more of the inputs are the same (ignoring the invert flag)
  3. the function itself ignores one or more of the inputs

So as we visit each node in the AST (from the bottom up), we will check for these reduction scenarios,
and reduce as appropriate. This will probably not get us very far.

Storing the AST in (fn, xs) format is interesting, because it allows us to deal with all nodes that share
the same function at once.

Shannon decomposition basically doubles the size of the AST at each step, so the idea is you'd probably
not actually double, but work one branch and then queue the other for later. But in solving one branch
recursively like this, you'll wind up doing a lot of work that you don't want to forget when you do the
other branch.

I started envisioning this scaffold concept as a way to avoid this work and not have to make two copies of
the whole AST. Instead, we introduce worlds that patch over the nodes we can directly simplify.

But this (fn,xs) idea opens up some other possibilities: we may start with simple functions of 2 or 3 values
(ands, ors, xors, maj, ite, etc.), but we can also meld these functions together to produce new functions of
more inputs. This melding step is intriguing because it does not necessarily double the size of the representation.
In other words, whenever two inputs to a node (f a b) have definitions which use overlapping vars
[for example, if (a = g x y , b = h y z) then a and b overlap on var y], then we can rewrite (f a b) as some
new function (f' x y z).

We can also easily query sets of nodes that use the same function, and modify them all in place.
So for example, we could build our AST such that all sums of n 32-bit values were just written
as abstract functions like (sum-n-bit-28). Then we could build the AST for that function exactly
once, and apply it to all instances simultaneously. Maybe even decompose it further, and have
a basic function like (parity-n) and (carry-n-0, carry-n-1, etc (for however many carry bits there are)).

This is nice because those functions aren't really order-dependent, and we might not need to represent
them with a BDD at all.

Another nice thing about this (fn,xs) representation is that we can permute the inputs of an individual
AST node however we like, without permuting the inputs globally, so we can rewrite it in whatever way
makes sense.

The whole time, we will have both AST nodes and scaffold nodes, and these can point to each other.

Is there a value to the chained-scope/worlds concept? Yes, it's a space saving device. Suppose we have
an extremely complicated AST to solve, and we want the shannon decomposition on x0. There may only be
a handful of leaf nodes that depend directly on x0, but modifying them would force us to rewrite the
entire AST. That's what we want to do eventually, but there's no point doing all that work if it's just
going to generate thousands of new nodes that don't actually get us closer to the result.

So instead, the "final BDD" that we want, will actually be a node that decides between two "worlds",
branching on x5 (since branching on x0..x4 already producesconstant NIDs).

So we have x5 ? World0 : World1.  Both of these are themselves chained to the underlying AST, and
they contain rewritten copies of *only* the nodes that rely directly on x5. This is a move-and-split
operation: the original definitions are removed from the AST, and so we can now only ever observe
them from some particular world.

This gives us a way to work bottom up while the substitution solver is working top-down to
convert the AST to a BDD/ANF.

This scaffold can also be used to construct an actual complete BDD with nothing but input
variables, by doing brute force (lo-to-hi) and/or monte-carlo evaluation. Basically, each node
in the "universe" branches between two worlds, and each world represents an AST, but once you've
dug deep enough along a path that all variables have been substituted, you will reach a world whose
AST is a constant. Once all the leaf nodes under a branch are 'constant worlds' like this, then
that branch is essentially pointing at a BDD.

As long as we are talking about AST nodes, we can imagine that each node is slowly being
transformed into a BDD, but also *consumed* by its downstream nodes, so we don't necessarily
need to keep the intermediate BDDs around. I've probably written about this idea before, but
basically it involves giving every node two "cursors". Everything to the left (lo side) of the
leftmost cursor has already been consumed, so we never need to evaluate it. Basically, once all
the directly-downstream neighbors of a node have moved their cursor past configuration x, there
is no need to ever evaluate anything to the left of x, so it might as well be erased. (This should
be attached to the AST nodes, not the BDD)







*/
