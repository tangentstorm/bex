"""
wrapper for bex to make it look like the dd package
https://github.com/tulip-control/dd/
"""
import bex as _bex
from typing import Any, Dict, Iterable, List, Optional, Set, Tuple, Union
from dd import _parser

class BDD:
    """dd-style python interface for bex::BddBase"""

    def __init__(self, name=None) -> None:
        """Initialize the BDD manager."""
        self.name = None  # for __str__
        self.base = _bex.BddBase()
        self.vars = {}
        self.var_count = 0

    @property
    def false(self) -> 'BDDNode':
        """Return the false constant (O)."""
        return BDDNode(self, _bex.O)

    @property
    def true(self) -> 'BDDNode':
        """Return the true constant (I)."""
        return BDDNode(self, _bex.I)

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
        return BDDNode(self, self.vars[name].to_nid())

    def _vhl(self, nid) -> Tuple[_bex.VID, _bex.NID, _bex.NID]:
        """Return the variable, high, and low nodes of a node."""
        return self.base.get_vhl(nid)

    def succ(self, u: 'BDDNode') -> Tuple[int, 'BDDNode', 'BDDNode']:
        """Return the successors of a node. (level, low, high)"""
        v,h,l = self._vhl(u.nid)
        return v.ix, BDDNode(self, l), BDDNode(self, h)

    def __eq__(self, other: Any) -> bool:
        """Check if two BDD managers are equal."""
        return isinstance(other, BDD) and self.base is other.base

    def _eval(self, nid: _bex.NID, assignment: Dict[_bex.VID, _bex.NID]) -> bool:
        return BDDNode(self, self.base.eval(nid, assignment))

    def _to_nid(self, x: Any) -> _bex.NID:
        if isinstance(x, bool):
            return _bex.I if x else _bex.O
        elif isinstance(x, str):
            return self.vars[x].to_nid()
        elif isinstance(x, BDDNode):
            return x.nid
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
        return self._to_vid(var).ix

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
        nid = self.base.ite(g.nid, u.nid, v.nid)
        return BDDNode(self, nid)

    def copy(self, u: 'BDDNode', other: 'BDD') -> 'BDDNode':
        """Copy a node from one BDD manager to another."""
        nid_map = {}
        for nid, v0, h0, l0 in self._walk_df(u.nid):
            v = BDDNode(other, v0.to_nid())
            # h and l should either be in nid_map or be literals
            h = nid_map.get(h0) or BDDNode(other, h0)
            l = nid_map.get(l0) or BDDNode(other, l0)
            print(v,h,l)
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
            yield {} # dd uses this to indicate al vars are "don't care"
        elif u.nid == _bex.O:
            return
        else:
            nvars = u.nid._vid().ix + 1
            cursor = self.base.first_solution(u.nid, nvars)
            while not cursor.at_end:
                ones = set(cursor.scope.hi_bits())
                yield {k: v.ix in ones for k, v in self.vars.items() if v.ix < nvars}
                cursor._advance(self.base)

    # -------------------------------------------------------------------------

    def configure(self, **kw: Any) -> Dict[str, Any]:
        """Configure the BDD manager with given parameters."""
        raise NotImplementedError("BDD.configure")

    def statistics(self) -> Dict[str, Any]:
        """Return statistics of the BDD manager."""
        raise NotImplementedError("BDD.statistics")

    def support(self, u: Any, as_levels: bool = False) -> Union[Set[str], Set[int]]:
        """Return the support of a node."""
        raise NotImplementedError("BDD.support")

    def forall(self, variables: Iterable[str], u: Any) -> Any:
        """Perform universal quantification on a node."""
        raise NotImplementedError("BDD.forall")

    def exist(self, variables: Iterable[str], u: Any) -> Any:
        """Perform existential quantification on a node."""
        raise NotImplementedError("BDD.exist")

    def pick(self, u: Any, care_vars: Optional[Set[str]] = None) -> Optional[Dict[str, bool]]:
        """Pick a satisfying assignment for a node."""
        raise NotImplementedError("BDD.pick")

    def to_expr(self, u: Any) -> str:
        """Convert a node to a Boolean expression."""
        raise NotImplementedError("BDD.to_expr")

    def _add_int(self, i: int) -> Any:
        """Add an integer to the BDD."""
        raise NotImplementedError("BDD._add_int")

    def cube(self, dvars: Dict[str, bool]) -> Any:
        """Return the conjunction of a set of literals."""
        raise NotImplementedError("BDD.cube")

    def dump(self, filename: str, roots: Optional[Union[Dict[str, Any], List[Any]]] = None, filetype: Optional[str] = None, **kw: Any) -> None:
        """Dump the BDD to a file."""
        raise NotImplementedError("BDD.dump")

    def load(self, filename: str, levels: bool = True) -> Union[Dict[str, Any], List[Any]]:
        """Load a BDD from a file."""
        raise NotImplementedError("BDD.load")


class BDDNode:
    """Pairs a NID with a reference to its BDD."""
    def __init__(self, bdd: BDD, nid: _bex.NID) -> None:
        """Initialize the BDDNode with a BDD and a node ID."""
        self.bdd = bdd
        self.nid = nid

    @property
    def vhl(self) -> Optional[Tuple[_bex.NID, _bex.NID]]:
        return self.bdd.base.get_vhl(self.nid)

    @property
    def high(self) -> Optional[_bex.NID]:
        return None if self.nid.is_const() else self.vhl[1]

    @property
    def low(self) -> Optional[_bex.NID]:
        return None if self.nid.is_const() else self.vhl[2]

    def __eq__(self, other: Any) -> bool:
        """Check if two BDDNodes are equal."""
        return isinstance(other, BDDNode) and self.bdd == other.bdd and self.nid == other.nid

    def __invert__(self) -> 'BDDNode':
        """Return the negation of the BDDNode."""
        return BDDNode(self.bdd, ~self.nid)

    def __and__(self, other: Any) -> 'BDDNode':
        """Return the conjunction of two BDDNodes."""
        return BDDNode(self.bdd, self.bdd.base.op_and(self.nid, other.nid))

    def __or__(self, other: Any) -> 'BDDNode':
        """Return the disjunction of two BDDNodes."""
        return BDDNode(self.bdd, self.bdd.base.op_or(self.nid, other.nid))

    def __xor__(self, other: Any) -> 'BDDNode':
        """Return the XOR of two BDDNodes."""
        return BDDNode(self.bdd, self.bdd.base.op_xor(self.nid, other.nid))

    def __str__(self) -> str:
        """Return a string representation of the BDDNode."""
        return f"BDDNode({self.bdd}, {self.nid})"
    
    def _vid(self) -> Optional[_bex.VID]:
        """Return the level of the BDDNode."""
        return None if self.nid.is_const() else self.nid._vid()

    # -------------------------------------------------------------------------

    def __call__(self, *args: Any, **kwargs: Any) -> Any:
        """Call the BDD function with given arguments."""
        raise NotImplementedError("BDDNode.__call__")

    def __hash__(self) -> int:
        """Return the hash of the BDDNode."""
        raise NotImplementedError("BDDNode.__hash__")

    def __repr__(self) -> str:
        """Return a string representation of the BDDNode for debugging."""
        raise NotImplementedError("BDDNode.__repr__")


def reorder(bdd: BDD, order: Optional[Dict[str, int]] = None) -> None:
    """Reorder the variables in the BDD."""
    raise NotImplementedError("reorder")
