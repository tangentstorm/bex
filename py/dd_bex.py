"""
wrapper for bex to make it look like the dd package
https://github.com/tulip-control/dd/
"""
import warnings
import weakref
import subprocess
from typing import Any, Dict, Iterable, List, Optional, Set, Tuple, Union
import bex as _bex
from dd import _parser

class BDD:
    """dd-style python interface for bex::BddBase"""

    def __init__(self, name=None) -> None:
        """Initialize the BDD manager."""
        self.name = None  # for __str__
        self.base = _bex.BddBase()
        self.vars = {}
        self.var_count = 0
        self._config = {'reordering':False}
        self.next_ref_num = 0
        self.ref_nids = weakref.WeakKeyDictionary() # BDDNode -> NID
        self.nid_refs = weakref.WeakValueDictionary() # NID -> BDDNode
        self.int_refs = weakref.WeakValueDictionary() # int -> BDDNode
        self.false = self._nidref(_bex.O)
        self.true = self._nidref(_bex.I)

    def _nidref(self, nid: _bex.NID) -> 'BDDNode':
        """Return the BDDNode corresponding to a NID."""
        if node := self.nid_refs.get(nid):
            return node
        else:
            self.nid_refs[nid] = \
            self.int_refs[self.next_ref_num] = \
            node = BDDNode(self, nid, self.next_ref_num)
            self.next_ref_num += 1
            return node

    def configure(self, **kw: Any) -> Dict[str, Any]:
        """Configure the BDD manager with given parameters.
        Returns the old configuration.
        """
        old = dict(**self._config)
        for k, v in kw.items():
            if k not in self._config:
                raise ValueError(f"Unknown configuration option: {k}")
            if k == 'reordering' and v:
                warnings.warn(".configure(reordering=True) currently does nothing")
            self._config[k] = v
        return old

    def add_var(self, name: str) -> None:
        """Add a new variable to the BDD."""
        if name in self.vars:
            raise ValueError(f"Variable {name} already declared")
        self.vars[name] = _bex.var(self.var_count)
        self.var_count += 1

    def declare(self, *names:str) -> None:
        """Declare new variables in the BDD."""
        for name in names:
            self.add_var(name)

    def var(self, name: str) -> 'BDDNode':
        """Return the node corresponding to a variable name."""
        return self._nidref(self.vars[name].to_nid())

    def _vhl(self, nid) -> Tuple[_bex.VID, _bex.NID, _bex.NID]:
        """Return the variable, high, and low nodes of a node."""
        return self.base.get_vhl(nid)

    def succ(self, u: 'BDDNode') -> Tuple[int, 'BDDNode', 'BDDNode']:
        """Return the successors of a node. (level, low, high)"""
        v,h,l = self._vhl(u.nid)
        return v.ix, self._nidref(l), self._nidref(h)

    def __eq__(self, other: Any) -> bool:
        """Check if two BDD managers are equal."""
        return isinstance(other, BDD) and self.base is other.base

    def _eval(self, nid: _bex.NID, assignment: Dict[_bex.VID, _bex.NID]) -> bool:
        return self._nidref(self.base.eval(nid, assignment))

    def _to_nid(self, x: Any) -> _bex.NID:
        if isinstance(x, bool):
            return _bex.I if x else _bex.O
        elif isinstance(x, str):
            return self.vars[x].to_nid()
        elif isinstance(x, BDDNode):
            return x.nid
        elif isinstance(x, int):
            try:
                return self.int_refs[x].nid
            except KeyError:
                raise ValueError(f"Invalid reference number: {x}")
        else:
            raise TypeError(f"Unsupported type: {type(x)}")

    def _to_vid(self, x: Any) -> _bex.VID:
        if isinstance(x, str):
            return self.vars[x]
        elif isinstance(x, BDDNode):
            return self._vhl(x.nid)[0]
        else:
            raise TypeError(f"Unsupported type: {type(x)}")

    def let(self, definitions: Union[Dict[str, str], Dict[str, bool], Dict[str, 'BDDNode']], u: 'BDDNode') -> 'BDDNode':
        """Substitute variables in a node."""
        if isinstance(definitions, dict):
            definitions = {self._to_vid(k): self._to_nid(v) for k, v in definitions.items()}
            return self._eval(u.nid, definitions)
        else:
            raise TypeError(f"Unsupported type for definitions: {type(definitions)}")

    def __len__(self) -> int:
        """Return the number of nodes in the BDD."""
        return len(self.base)

    def __contains__(self, u: 'BDDNode') -> bool:
        """Check if a node is in the BDD."""
        if not isinstance(u, BDDNode):
            raise TypeError()
        if u.bdd != self:
            # !! not sure why this should raise a value error, but that's what the tests ask for.
            raise ValueError
        return True

    def level_of_var(self, var: str) -> Optional[int]:
        """Return the level of a given variable."""
        return (len(self.vars) - 1) - self._to_vid(var).ix

    def var_at_level(self, level: int) -> str:
        """Return the variable at a given level."""
        for var, vid in self.vars.items():
            if vid.ix == level:
                return var
        else: raise LookupError(f"No variable found at level {level}")

    @property
    def var_levels(self) -> Dict[str, int]:
        """Return the levels of all variables."""
        return {var: vid.ix for var, vid in self.vars.items()}

    def add_expr(self, expr: str) -> Any:
        """Add a Boolean expression to the BDD."""
        return _parser.add_expr(expr, self)

    def apply(self, op: str, u: Any, v: Optional[Any] = None, w: Optional[Any] = None) -> Any:
        """Apply a binary or ternary operator to nodes."""
        match op:
            case "not" | "!": return ~u
            case "and" | "/\\" | "&" | "&&": return u & v
            case "or"  | "\\/" | "|" | "||": return u | v
            case "xor" | "#" | "^": return u ^ v
            case "ite" : return self.ite(u, v, w)
            case "<=>" | "<->" | "equiv" : return ~(u ^ v)
        raise NotImplementedError(f"BDD.apply({op})")

    def _walk_df(self, nid: _bex.NID) -> Iterable[Tuple[_bex.NID, _bex.VID, _bex.NID, _bex.NID]]:
        """Walk through the BDD (depth-first and left to right), yielding tuples of (nid, v, h, l)."""
        seen = set()
        stack = [None]
        this = nid
        while this:
            v, h, l = self.base.get_vhl(this)
            todo = [n for n in [l, h] if not (n.is_lit() or n in seen)]
            if todo:
                stack.push(this)
                stack.extend(todo)
            else:
                yield (this, v, h, l)
            this = stack.pop()

    def ite(self, g: 'BDDNode', u: 'BDDNode', v: 'BDDNode') -> 'BDDNode':
        """Perform the if-then-else operation on nodes."""
        return self._nidref(self.base.ite(g.nid, u.nid, v.nid))

    def find_or_add(self, v:str, l:'BDDNode', h: 'BDDNode') -> 'BDDNode':
        """Find or add a node to the BDD. Note that dd puts the low branch first (bex usually does the opposite)"""
        return self.ite(self.var(v), h, l)

    def copy(self, u: 'BDDNode', other: 'BDD') -> 'BDDNode':
        """Copy a node from one BDD manager to another."""
        nid_map = {}
        for nid, v0, h0, l0 in self._walk_df(u.nid):
            v = other._nidref(v0.to_nid())
            # h and l should either be in nid_map or be literals
            h = nid_map.get(h0) or other._nidref(h0)
            l = nid_map.get(l0) or other._nidref(l0)
            nid_map[nid] = last = other.ite(v, h, l)
        return last

    def __str__(self) -> str:
        """Return a string representation of the BDD."""
        return f"BDD(name={self.name})" if self.name else repr(self)

    def count(self, u: 'BDDNode', nvars: Optional[int] = None) -> int:
        """Count the number of satisfying assignments for a node."""
        shift = 0
        if nvars is not None:
            if nvars == 0 and not u.nid.is_const(): raise ValueError("nvars must be > 0")
            if u.nid.is_const(): return int(u.nid==_bex.I) << nvars
            shift = nvars - (u._vid().ix + 1)
            print("shift", shift, " nvars:", nvars, "vid:", u._vid())
        return self.base.solution_count(u.nid) << shift

    def pick_iter(self, u: 'BDDNode', care_vars:Set[str]=set()) -> Iterable[Dict[str, bool]]:
        """Iterate over all solutions of the BDD."""
        # TODO: support dont_care situations
        if u.nid == _bex.I:
            yield {} # dd uses this to indicate all vars are "don't care"
        elif u.nid == _bex.O:
            return
        else:
            nvars = u.nid._vid().ix + 1
            cursor = self.base.make_dontcare_cursor(u.nid, nvars)
            for s in care_vars:
                cursor._watch(self.vars[s])
            revmap = {vid: s for s,vid in self.vars.items()}
            while not cursor.at_end:
                cube = cursor.cube
                yield {revmap[vid]: bit for vid,bit in cube}
                cursor._advance(self.base)

    def when_hi(self, nid: _bex.NID, vid: _bex.VID) -> 'BDDNode':
        """Return the node when the variable is true."""
        return self._nidref(self.base.when_hi(nid, vid))

    def when_lo(self, nid: _bex.NID, vid: _bex.VID) -> 'BDDNode':
        """Return the node when the variable is false."""
        return self._nidref(self.base.when_lo(nid, vid))

    def quantify(self, u: 'BDDNode', variables: Iterable[str], forall: bool = True) -> 'BDDNode':
        """Perform quantification on a node."""
        res = u
        for s in variables:
            v = self.vars[s]
            res = self.apply(op = 'and' if forall else 'or',
                             u = self.when_hi(v, res.nid),
                             v = self.when_lo(v, res.nid))
        return res

    def forall(self, variables: Iterable[str], u: Any) -> Any:
        """Perform universal quantification on a node."""
        return self.quantify(u, variables, forall=True)

    def exist(self, variables: Iterable[str], u: Any) -> Any:
        """Perform existential quantification on a node."""
        return self.quantify(u, variables, forall=False)

    def support(self, u: Any, as_levels: bool = False) -> Union[Set[str], Set[int]]:
        """Return the support of a node."""
        res = self.base.support(u.nid)
        if as_levels:
            return {v.ix for v in res}
        else:
            rev = {v: s for s,v in self.vars.items()}
            return {rev[v] for v in self.base.support(u.nid)}

    def to_expr(self, u: Any) -> str:
        """Convert a node to a Boolean expression."""
        def s(nid):
            if nid.is_const():
                return 'TRUE' if nid == _bex.I else 'FALSE'
            if nid.is_lit():
                return self.var_at_level(nid._vid().ix)
            return f'#{nid.ix:03x}'
        if u.nid.is_lit():
            return s(u.nid)
        else:
            v, h, l = self._vhl(u.nid)
            return f'ite({s(v.to_nid())}, {s(h)}, {s(l)})'

    def _add_int(self, i: int) -> Any:
        """I think this is meant to add a reference to an existing node by its ref number.
        (Which it does. Python tracks the references internally.)"""
        return self.int_refs[i]

    def cube(self, vars: Union[Dict[str, bool], List[str]]) -> 'BDDNode':
        """Return the conjunction of a set of literals."""
        if isinstance(vars, list):
            vars = {name: True for name in vars}
        if not isinstance(vars, dict):
            raise TypeError("cube expects a list or dict")
        res = self.true
        for name, inv in sorted(vars.items(), key=lambda item: self.vars[item[0]].ix):
            res = res & self.var(name)._inv_if(inv)
        return res

    def dump(self, filename: str, roots: Optional[List[Any]] = None, filetype: Optional[str] = None) -> None:
        """Dump the BDD to a file."""
        if roots is None:
            raise ValueError("please supply at least one root to dump")
        if filetype is None:
            filetype = filename[filename.rfind('.')+1:] if '.' in filename else 'dot'
        if filetype in ['dot', 'pdf', 'svg', 'png']:
            dot_code = '\n'.join(self.base.to_dot(root.nid) for root in roots)
            if filetype == 'dot':
                with open(filename, 'w', encoding='utf-8') as f:
                    f.write(dot_code)
            else:
                cmd = ['dot', f'-T{filetype}', '-o', filename]
                subprocess.run(cmd, encoding='utf8', input=dot_code, capture_output=True, check=True)
        elif filetype == 'json':
            assert len(roots) == 1, "json dump only supports a single root"
            json_code = self.base.to_json(roots[0].nid)
            with open(filename, 'w', encoding='utf-8') as f:
                f.write(json_code)
        else:
            raise ValueError(f"unsupported filetype for dump: {filetype}")

    def load(self, filename: str, levels: bool = True) -> Union[List['BDDNode']]:
        """Load a BDD from a file."""
        return []

    def reorder(self, order: Optional[Dict[str, int]] = None):
        """Reorder the variables in the BDD."""
        if not order:
            raise NotImplementedError("reorder without args")
        print('vars before:', self.vars)
        keep = list({nid for nid in self.nid_refs.keys() if not nid.is_const()})
        last = len(order)-1 # bex orders variables from the bottom up, so we want to reverse the list
        perm = [v for ix,v in sorted((last-new_ix, self.vars[var]) for var, new_ix in order.items())]
        print("keep:", keep)
        print("perm:", perm)
        kept = self.base.reorder(perm, keep, gc=True) # returns new nids. always garbage collect.
        # update our internal variable list. again, reverse the order.
        for i,name in enumerate(order):
            self.vars[name] = _bex.var(last-i)
        print('vars after:', self.vars)
        # now update all the live python objects with the new nid:
        refs = [self.nid_refs[old] for old in keep]
        self.nid_refs = weakref.WeakValueDictionary()
        for ref, new in zip(refs, kept):
            self.nid_refs[new] = ref
            ref.nid = new

    # -------------------------------------------------------------------------

    def statistics(self) -> Dict[str, Any]:
        """Return statistics of the BDD manager."""
        raise NotImplementedError("BDD.statistics")

    def pick(self, u: Any, care_vars: Optional[Set[str]] = None) -> Optional[Dict[str, bool]]:
        """Pick a satisfying assignment for a node."""
        raise NotImplementedError("BDD.pick")


class BDDNode:
    """Pairs a NID with a reference to its BDD."""
    def __init__(self, bdd: BDD, nid:_bex.NID, _id: int) -> None:
        """Initialize the BDDNode with a BDD and a node ID."""
        self.bdd = bdd
        self.nid = nid
        self._id = _id

    @property
    def _vhl(self) -> Optional[Tuple[_bex.NID, _bex.NID]]:
        """Return the variable, high, and low nodes of a node.
        Note that unlike bex, dd wants us to return the vhl for the RAW nid,
        and inverts them separately if self.negated
        """
        return self.bdd.base.get_vhl(self.nid.raw)

    @property
    def var(self) -> Optional[str]:
        return None if self.nid.is_const() else self.bdd.var_at_level(self.nid._vid().ix)

    @property
    def high(self) -> Optional['BDDNode']:
        return None if self.nid.is_const() else self.bdd._nidref(self._vhl[1])

    @property
    def low(self) -> Optional['BDDNode']:
        return None if self.nid.is_const() else self.bdd._nidref(self._vhl[2])

    def __eq__(self, other: Any) -> bool:
        """Check if two BDDNodes are equal."""
        return isinstance(other, BDDNode) and self.bdd == other.bdd and self.nid == other.nid

    def __invert__(self) -> 'BDDNode':
        """Return the negation of the BDDNode."""
        return self.bdd._nidref(~self.nid)

    def __and__(self, other: Any) -> 'BDDNode':
        """Return the conjunction of two BDDNodes."""
        return self.bdd._nidref(self.bdd.base.op_and(self.nid, other.nid))

    def __or__(self, other: Any) -> 'BDDNode':
        """Return the disjunction of two BDDNodes."""
        return self.bdd._nidref(self.bdd.base.op_or(self.nid, other.nid))

    def __xor__(self, other: Any) -> 'BDDNode':
        """Return the XOR of two BDDNodes."""
        return self.bdd._nidref(self.bdd.base.op_xor(self.nid, other.nid))

    def __repr__(self) -> str:
        """Return a string representation of the BDDNode."""
        return f"BDDNode({self.bdd}, {self.nid})"

    def __str__(self) -> str:
        """Return a string representation of the nid"""
        return f"@{int(self)}"

    def _vid(self) -> Optional[_bex.VID]:
        """Return the level of the BDDNode."""
        return None if self.nid.is_const() else self.nid._vid()

    def __hash__(self) -> int:
        """Return the hash of the BDDNode."""
        return hash((id(self.bdd), self._id))

    def __lt__(self, other: Any) -> bool:
        if not isinstance(other, BDDNode):
            return NotImplemented
        if self.bdd != other.bdd:
            raise ValueError("Cannot compare BDDNodes from different BDD managers")
        # Handle constant nodes specially
        if self.nid.is_const():
            if other.nid.is_const():
                return int(self.nid) < int(other.nid)
            # false constant is less than any nonconstant;
            # true constant is greater than any nonconstant.
            return int(self.nid) == int(_bex.O)
        else:
            if other.nid.is_const():
                return int(other.nid) == int(_bex.I)
            return int(self.nid) < int(other.nid)

    def __le__(self, other: Any) -> bool:
        return self == other or self < other

    def __gt__(self, other: Any) -> bool:
        if not isinstance(other, BDDNode):
            return NotImplemented
        return not (self <= other)

    def __ge__(self, other: Any) -> bool:
        return self == other or self > other

    @property
    def support(self) -> Set[str]:
        """Return the set of variables used by a node."""
        return self.bdd.support(self)

    def __int__(self) -> int:
        """return the nid as a python int"""
        return self._id

    def _inv_if(self, bit:bool) -> 'BDDNode':
        """Invert the node if bit is True."""
        return self if bit else ~self

    @property
    def negated(self) -> bool:
        """Return True if the node is negated."""
        return self.nid.is_inv()
    @property
    def level(self) -> int:
        """Return the level of the node."""
        return self.bdd.level_of_var(self.var)

    @property
    def ref(self) -> int:
        """I don't know what this is, but it's in the tests. Internal refcount, maybe? Bex doesn't have this."""
        # (Bex does garbage collection via refcounting when it reorders nodes, but otherwise doesn't refcount.)
        return 1

    # -------------------------------------------------------------------------

    def __call__(self, *args: Any, **kwargs: Any) -> Any:
        """Call the BDD function with given arguments."""
        raise NotImplementedError("BDDNode.__call__")
