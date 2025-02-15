import dd_bex as _bdd

# these are in the 'tests' directory in the dd package.
# (i just added that directory to my PYTHONPATH for now)
import common
import common_bdd

class Tests(common_bdd.Tests, common.Tests):
    def setup_method(self):
        self.DD = _bdd.BDD

    def test_len(self):
        """bex does not allocate nodes for constants or literals"""
        bdd = self.DD()
        u = bdd.true
        # assert len(bdd) == 1, len(bdd)
        assert len(bdd) == 0, len(bdd)

    def test_to_expr(self):
        """bex orders variables from the bottom up"""
        bdd = self.DD()
        bdd.declare('x', 'y')
        u = bdd.var('x')
        r = bdd.to_expr(u)
        r_ = 'x'
        assert r == r_, (r, r_)
        u = bdd.add_expr(r'x /\ y')
        r = bdd.to_expr(u)
        # r_ = 'ite(x, y, FALSE)'
        r_ = 'ite(y, x, FALSE)'
        assert r == r_, (r, r_)
        u = bdd.add_expr(r'x \/ y')
        r = bdd.to_expr(u)
        # r_ = 'ite(x, TRUE, y)'
        r_ = 'ite(y, TRUE, x)'
        assert r == r_, (r, r_)
