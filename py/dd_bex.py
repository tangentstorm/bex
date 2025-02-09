"""
wrapper for bex to make it look like the dd package
https://github.com/tulip-control/dd/
"""
import bex as _bex
from typing import Any, Dict, Iterable, List, Optional, Set, Tuple, Union

class BDD:
    def __init__(self) -> None:
        """Initialize the BDD manager."""
        raise NotImplementedError("BDD.__init__")

    def __eq__(self, other: Any) -> bool:
        """Check if two BDD managers are equal."""
        raise NotImplementedError("BDD.__eq__")

    def __len__(self) -> int:
        """Return the number of nodes in the BDD."""
        raise NotImplementedError("BDD.__len__")

    def __contains__(self, u: Any) -> bool:
        """Check if a node is in the BDD."""
        raise NotImplementedError("BDD.__contains__")

    def __str__(self) -> str:
        """Return a string representation of the BDD."""
        raise NotImplementedError("BDD.__str__")

    def configure(self, **kw: Any) -> Dict[str, Any]:
        """Configure the BDD manager with given parameters."""
        raise NotImplementedError("BDD.configure")

    def statistics(self) -> Dict[str, Any]:
        """Return statistics of the BDD manager."""
        raise NotImplementedError("BDD.statistics")

    def succ(self, u: Any) -> Tuple[int, Optional[Any], Optional[Any]]:
        """Return the successors of a node."""
        raise NotImplementedError("BDD.succ")

    def declare(self, *variables: str) -> None:
        """Declare new variables in the BDD."""
        raise NotImplementedError("BDD.declare")

    def var(self, var: str) -> Any:
        """Return the node corresponding to a variable."""
        raise NotImplementedError("BDD.var")

    def var_at_level(self, level: int) -> str:
        """Return the variable at a given level."""
        raise NotImplementedError("BDD.var_at_level")

    def level_of_var(self, var: str) -> Optional[int]:
        """Return the level of a given variable."""
        raise NotImplementedError("BDD.level_of_var")

    @property
    def var_levels(self) -> Dict[str, int]:
        """Return the levels of all variables."""
        raise NotImplementedError("BDD.var_levels")

    def copy(self, u: Any, other: 'BDD') -> Any:
        """Copy a node from one BDD manager to another."""
        raise NotImplementedError("BDD.copy")

    def support(self, u: Any, as_levels: bool = False) -> Union[Set[str], Set[int]]:
        """Return the support of a node."""
        raise NotImplementedError("BDD.support")

    def let(self, definitions: Union[Dict[str, str], Dict[str, bool], Dict[str, Any]], u: Any) -> Any:
        """Substitute variables in a node."""
        raise NotImplementedError("BDD.let")

    def forall(self, variables: Iterable[str], u: Any) -> Any:
        """Perform universal quantification on a node."""
        raise NotImplementedError("BDD.forall")

    def exist(self, variables: Iterable[str], u: Any) -> Any:
        """Perform existential quantification on a node."""
        raise NotImplementedError("BDD.exist")

    def count(self, u: Any, nvars: Optional[int] = None) -> int:
        """Count the number of satisfying assignments for a node."""
        raise NotImplementedError("BDD.count")

    def pick(self, u: Any, care_vars: Optional[Set[str]] = None) -> Optional[Dict[str, bool]]:
        """Pick a satisfying assignment for a node."""
        raise NotImplementedError("BDD.pick")

    def pick_iter(self, u: Any, care_vars: Optional[Set[str]] = None) -> Iterable[Dict[str, bool]]:
        """Return an iterator over satisfying assignments for a node."""
        raise NotImplementedError("BDD.pick_iter")

    def add_expr(self, expr: str) -> Any:
        """Add a Boolean expression to the BDD."""
        raise NotImplementedError("BDD.add_expr")

    def to_expr(self, u: Any) -> str:
        """Convert a node to a Boolean expression."""
        raise NotImplementedError("BDD.to_expr")

    def ite(self, g: Any, u: Any, v: Any) -> Any:
        """Perform the if-then-else operation on nodes."""
        raise NotImplementedError("BDD.ite")

    def apply(self, op: str, u: Any, v: Optional[Any] = None, w: Optional[Any] = None) -> Any:
        """Apply a binary or ternary operator to nodes."""
        raise NotImplementedError("BDD.apply")

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

    @property
    def false(self) -> Any:
        """Return the false constant."""
        raise NotImplementedError("BDD.false")

    @property
    def true(self) -> Any:
        """Return the true constant."""
        raise NotImplementedError("BDD.true")

def reorder(bdd: BDD, order: Optional[Dict[str, int]] = None) -> None:
    """Reorder the variables in the BDD."""
    raise NotImplementedError("reorder")
