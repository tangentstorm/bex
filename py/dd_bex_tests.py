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


class BDDTests(common_bdd.Tests):
    def setup_method(self):
        self.DD = _bdd.BDD

