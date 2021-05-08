"""
This library generates the graph diagrams in doc/scaffold.ipynb,
as well as corresponding test cases in src/test-swap-scaffold.rs
"""
from collections import Counter
from itertools import tee
from typing import Union as U
import graphviz

# so we don't get random memory addresses in the git diff every time this saves:
setattr(graphviz.Graph, '__repr__', lambda self:'[Graph]')
setattr(graphviz.Digraph, '__repr__', lambda self:'[Digraph]')

## test generation ######################################################################

# we're going to batch the tests up up and write them at the end
RUST_TESTS = []  # [{setup:str, check:str, label:str}]

def write_tests():
    with open('../src/test-swap-scaffold.rs','w',newline='\n') as o:
        o.write('///! test suite generated from doc/scaffold.ipynb\n\n')
        o.write('#[cfg(test)] use std::iter::FromIterator;\n\n')
        for i, item in enumerate(RUST_TESTS):
            o.write('\n'.join([
                f'/// test for diagram #{i}: {item["label"]}',
                f'#[allow(unused_variables)]',
                f'#[test] fn test_scaffold_diagram{i}() {{',
"""
    let mut xs = XVHLScaffold::new();
    // arbitrary fake vars for the fake nodes to reference. (these go at the bottom)
    let vn0 = VID::var(0); xs.push(vn0); let vx0 = VID::var(20); xs.push(vx0);
    let vn1 = VID::var(1); xs.push(vn1); let vx1 = VID::var(21); xs.push(vx1);
    let vn2 = VID::var(2); xs.push(vn2); let vx2 = VID::var(22); xs.push(vx2);
    let vn3 = VID::var(3); xs.push(vn3); let vx3 = VID::var(23); xs.push(vx3);
    let vn4 = VID::var(4); xs.push(vn4);
    let vn5 = VID::var(5); xs.push(vn5);
    let vn6 = VID::var(6); xs.push(vn6);
    let vn7 = VID::var(7); xs.push(vn7);
    let vn8 = VID::var(8); xs.push(vn8);
    let vn9 = VID::var(9); xs.push(vn9);

    // variables used in the swap tests. These look "upside down" here
    // because we're pushing them onto a stack. Remember: vu starts below vd.
    let va = VID::vir(0); xs.push(va);
    let vu = VID::vir(1); xs.push(vu);
    let vd = VID::vir(2); xs.push(vd);
    let vz = VID::vir(4); xs.push(vz);

    // constructors for default nodes
    assert_eq!(1, xs.vhls.len(), "expecting only XVHL_O at this point");
    let mut node = |v,hi,lo|->XID { xs.add_ref(XVHL{v, hi, lo}, 0, 0) };
    const XO:XID = XID_O;
    const SLOTS:usize = 9;

    // leave some space for the numbered nodes in the diagrams by creating fake nodes:
    // (can't use XID_O because add_ref would overwrite the empty slot)
    let (n1,n2,n3) = (node(vn1,XO,!XO), node(vn2,XO,!XO), node(vn3,XO,!XO));
    let (n4,n5,n6) = (node(vn4,XO,!XO), node(vn5,XO,!XO), node(vn6,XO,!XO));
    let (n7,n8,n9) = (node(vn7,XO,!XO), node(vn8,XO,!XO), node(vn9,XO,!XO));

    // now some fake nodes for the a/z rows to point at when the edges are not defined:
    let (x0,x1,x2,x3) = (node(vx0,XO,!XO), node(vx1,XO,!XO), node(vx2,XO,!XO), node(vx3,XO,!XO));

    // and the default a and z rows themselves:
    let (z0,z1,z2,z3) = (node(vz,x0,!x0), node(vz,x1,!x1), node(vz,x2,!x2), node(vz,x3,!x3));
    let (a0,a1,a2,a3) = (node(va,x0,!x0), node(va,x1,!x1), node(va,x2,!x2), node(va,x3,!x3));

    // setup code generated from the diagram:
""",
                item["setup"],
"""
    // go back and clear out the fake nodes we created earlier:
    let mut i = 1;
    while i <= SLOTS && xs.vhls[i].v.is_vir() { i+=1 }
    while i <= SLOTS { xs.del_node(XID{x:i as i64}); i+=1; }

    // double check that the diagram itself follows the rules.
    xs.validate("setup from diagram");

    // now perform the swap:
    xs.swap(vu);

    // checks generated from the diagram:
""",
                item["check"],
"""
}
""",
                f'\n\n']))

def xid(x:U[int,str])->str:
    """convert the graph label to a XID var used in the test suite (defined in src/test-swap.rs)"""
    return f"n{abs(x)}" if type(x) == int else "un" if x=="*" else x

def make_vmap(ru,rd):
    """return a dict mapping ids in the notation to their rust variable names"""
    r = {x:'v'+x[0] for x in "z0 z1 z2 z3 a0 a1 a2 a3".split()}
    r.update({x:'vu' for x in ru})
    r.update({x:'vd' for x in rd})
    return r

def make_xvhls(ru,rd,ites):
    """converts the notation to (x:XID, v:VID, hi:XID, lo:XID) tuples (in rust syntax)"""
    ru = [abs(x) for x in ru]
    rd = [abs(x) for x in rd]
    vmap = make_vmap(ru,rd)
    # !! the nodes have to be built in order from the bottom up,
    #    so that we don't delete a node after pointing an edge to it.
    # !! I think it reads better from top to bottom though (that's how I wrote them)
    #    so for now, you have to just manually write the ite triples from top to bottom in the spec.
    seen = {'a0','a1','a2','a3','un'}    # this is so we can at least warn if you use the wrong order.
    for i, t, e in reversed(ites):
        v, x, hi, lo = vmap[i], xid(i), xid(t), xid(e)
        if hi in seen and lo in seen:
            seen.add(x)
            yield x,v,hi,lo
        else: raise ValueError(f'ites must be specified from top to bottom for now! (bad: {repr((i,t,e))})')

def rust_scaffold_setup(ru,rd,ites):
    """used to translate the "before" diagram into code to set up the scaffold"""
    for x, v, hi, lo in make_xvhls(ru,rd,ites):
        yield f'  let (old_xid, old_vhl) = ({x}, xs.get({x}).unwrap()); xs.del_node({x});'
        if hi == 'un': hi = 'old_vhl.hi'
        if lo == 'un': lo = 'old_vhl.lo'
        yield f'  let {x} = xs.add_ref(XVHL{{ v:{v}, hi:{hi}, lo:{lo} }}, 0, 0);'
        yield f'  assert_eq!({x}.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for {x}.");'

def rust_scaffold_check(ru,rd,ites):
    """used to translate the "after" diagram into a set of rust assertions to make after calling swap()"""
    refs = Counter()
    # check that the nodes on rows u and d match the diagram exactly:
    for v, row in [('u',ru), ('d',rd)]:
        actual = f"xs.xids_on_row(v{v})"
        expect = f"HashSet::from_iter(vec![{', '.join(xid(x) for x in row)}])"
        yield f'  assert_eq!({actual}, {expect}, "row {v} didn\'t match expected values!");'
    # do our own refcount based on the diagram, and check that the vhl entry matches:
    for x, v, hi, lo in make_xvhls(ru,rd,ites):
        refs[hi]+=1
        refs[lo]+=1
        yield f' {{ let x=xs.get({x}).unwrap();'
        if hi != 'un': yield f'    assert_eq!( x.hi, {hi}, "wrong .hi for node {x}");'
        if lo != 'un': yield f'    assert_eq!( x.lo, {lo}, "wrong .hi for node {x}");'
        yield f'    assert_eq!(x.v, {v}, "wrong variable for node {x}: {{:?}}", {x}); }}'
    # finally, check the refcounts:
    for x, rc in refs.items():
        if x == 'un': continue
        yield f'  assert_eq!(xs.get_refcount({x}).unwrap(), {rc}, "bad refcount for node {x} ({{:?}})!", {x});'

def test_ite_scaffold(label, before, after):
    RUST_TESTS.append({
        'label': label,
        'setup': '\n'.join(rust_scaffold_setup(**before)),
        'check': '\n'.join(rust_scaffold_check(**after)) })

def ite_scaffold(label, before, after):
    test_ite_scaffold(label, before, after)
    return draw_ite_scaffold(label, before, after)


## drawing support ######################################################################

FADE = "#cccccc"
TEXT = "black"
INVIS = "white"

def pairs(xs):
    xs, ys = tee(xs)
    next(ys, None)
    return zip(xs, ys)


def add_row(g, n, row, active, nodes, **kw):
    with g.subgraph(name=n+row) as c:
        c.attr(rank='same', pencolor=FADE)
        pcolor = FADE
        if row in 'ud':
            pcolor = 'black'
            bcolor = '#cc9999' if row == 'u' else '#9999cc'
            c.attr(style='filled', color=bcolor, fontcolor='black', pencolor=pcolor)
        c.attr('node', **kw)
        nodes = [(x,x) if isinstance(x,str) else x for x in nodes]
        for i,(x,lbl) in enumerate(nodes):
            fcolor = 'orange' if row =='u' else 'dodgerblue' if row=='d' else 'white'
            ncolor = 'black' if row in 'ud' else FADE if x in active else INVIS
            tcolor = ncolor if row in 'az' else TEXT
            c.node(n+x, label=lbl, group=row, style='filled',
                   color=ncolor, fillcolor=fcolor, fontcolor=tcolor)
        # force them to flow left-to right
        if row in 'az' or (row=='u' and n=='a') or (row=='d' and n=='b'):
            for x,y in pairs([n+row]+[n+x[0] for x in nodes]):
                c.edge(x,y,style='invis')
        prime = "'" if n=='a' and row in 'ud' else ""
        c.node(n+row, label=row+prime, width="1", shape='none', fontcolor=pcolor, group='clusters')


def edge_color(v):
    return FADE if v[0] in 'az' else 'black'

def add_ite(g,n, v, hi, lo):
    if hi!="*": g.edge(n+v, n+hi, style='solid', color=edge_color(v))
    if lo!="*": g.edge(n+v, n+lo, style='dashed', color=edge_color(v))

def row_order(g,n, cs):
    for x, y in pairs(cs):
        g.edge(n+x, n+y, style='invis')

def node_label(x:U[str,int])->(str,str):
    if type(x) == str: return x, x
    else: return f'n{x}', f'#{x}'


def draw_scaffold(g,n, label, seq, ru=(), rd=(), ites=(), **kw):

    g.attr(rankdir='TB', labeljust='l', newrank='true', remincross='false',
           pencolor="#666666", label=label, ranksep="0.25")
    g.attr('edge', arrowsize='0.75')
    g.attr('node', shape='circle', width="0.4", fixedsize="true")

    active = {node_label(x)[0] for vhl in ites for x in vhl}
    add_row(g, n, 'z', active, ['z0', 'z1', 'z2', 'z3'], color=FADE, fontcolor=FADE)
    add_row(g, n, 'd', active, [node_label(abs(x)) for x in rd])
    add_row(g, n, 'u', active, [node_label(abs(x)) for x in ru])
    add_row(g, n, 'a', active, ['a0', 'a1', 'a2', 'a3'], color=FADE, fontcolor=FADE)
    row_order(g,n, seq)

    for ite in ites:
        add_ite(g,n, *(node_label(x)[0] for x in ite))


def draw_ite_scaffold(label, before, after):
    d = graphviz.Digraph()
    d.attr(label=f"diagram {len(RUST_TESTS)-1}. {label}")
    with d.subgraph(name="cluster_before") as g:
        draw_scaffold(g,'b', 'before', 'zdua', **before)
    with d.subgraph(name="cluster_after") as g:
        g.attr(label='after', pencolor='blue')
        draw_scaffold(g,'a', 'after', 'zuda', **after)
    # print(d.source)
    return d



# constants for the top and bottom rows
z0, z1, z2, z3="z0 z1 z2 z3".split()
a0, a1, a2, a3="a0 a1 a2 a3".split()
un="*" # "undefined" (when we don't care where an edge goes)
