from bex import I, O, vir, var, AST
assert str(O)=="O"
assert str(I)=="I"
assert str(vir(0))=="v0"
assert str(var(0))=="x0"

x0, x1, x2 = [var(x) for x in range(3)]

base = AST()
n0 = base.op_and(x0, x1)
n1 = base.op_or(x2, n0)

dot = base.to_dot(n1)
print("(x0 & x1) | x2 :\n\n", dot)
