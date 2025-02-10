"""
sanity check for python interface to bex.
for a real test suite, see the dd wrapper (dd_bex_tests)
"""
# TODO: make a real test suite here
from bex import I, O, vir, var, nvar, ASTBase
assert str(O)=="O"
assert str(I)=="I"
assert str(vir(0))=="v0"
assert str(var(0))=="x0"

x0, x1, x2 = [var(x) for x in range(3)]

nx0 = x0.to_nid()
nx1 = nvar(1)
nx2 = nvar(2)

base = ASTBase()
n0 = base.op_and(nx0, nx1)
n1 = base.op_or(nx2, n0)

dot = base.to_dot(n1)
print("(x0 & x1) | x2 :\n\n", dot)
