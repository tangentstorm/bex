import logging

import dd_bex as _bdd
import dd._utils as _utils
import pytest

# these are in the 'tests' directory in the dd package.
# (i just added that directory to my PYTHONPATH for now)
import common
import common_bdd


logging.getLogger('astutils').setLevel('ERROR')


class Tests(common.Tests):
    def setup_method(self):
        self.DD = _bdd.BDD

    @pytest.mark.skip(reason="not implemented")
    def test_configure_reordering(self):
        pass

    def test_len(self):
        """bex does not allocate nodes for constants or literals"""
        bdd = self.DD()
        u = bdd.true
        assert len(bdd) == 0, len(bdd)

class BDDTests(common_bdd.Tests):
    def setup_method(self):
        self.DD = _bdd.BDD

