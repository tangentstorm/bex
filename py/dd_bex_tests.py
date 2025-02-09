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


class BDDTests(common_bdd.Tests):
    def setup_method(self):
        self.DD = _bdd.BDD

