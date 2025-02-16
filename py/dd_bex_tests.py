import dd_bex as _bdd

# these are in the 'tests' directory in the dd package.
# (i just added that directory to my PYTHONPATH for now)
import common
import common_bdd

class Tests(common_bdd.Tests, common.Tests):
    """
    The tests in this file override tests in the dd modules,
    because whereas most BDD packages number variables from the
    top down, bex numbers them from the bottom up. Also, bex
    does not need to allocate nodes for constants or literals.
    """

    def setup_method(self):
        self.DD = _bdd.BDD

    def test_len(self):
        """bex does not allocate nodes for constants or literals"""
        bdd = self.DD()
        u = bdd.true
        # -- assert len(bdd) == 1, len(bdd)
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
        # -- r_ = 'ite(x, y, FALSE)'
        # !! the branch will be on y in bex because y=1, x=0, and 1>0
        r_ = 'ite(y, x, FALSE)'
        assert r == r_, (r, r_)
        u = bdd.add_expr(r'x \/ y')
        r = bdd.to_expr(u)
        # r_ = 'ite(x, TRUE, y)'
        r_ = 'ite(y, TRUE, x)'
        assert r == r_, (r, r_)

    def test_function_properties(self):
        bdd = self.DD()
        bdd.declare('x', 'y')
        order = dict(x=0, y=1)
        bdd.reorder(order)
        u = bdd.add_expr(r'x \/ y')
        # -- y = bdd.add_expr('y')
        x = bdd.add_expr('x')
        # Assigned first because in presence of a bug
        # different property calls could yield
        # different values.
        level = u.level
        # -- assert level == 0, level
        # !! the level in bex is the variable with the highest number
        assert level == 1, level
        var = u.var
        # -- assert var == 'x', var
        assert var == 'y', var
        low = u.low
        # -- assert low == y, low
        assert low == x, low
        high = u.high
        assert high == bdd.true, high
        ref = u.ref
        assert ref == 1, ref
        assert not u.negated
        support = u.support
        assert support == {'x', 'y'}, support
        # terminal
        u = bdd.false
        assert u.var is None, u.var
        assert u.low is None, u.low
        assert u.high is None, u.high
