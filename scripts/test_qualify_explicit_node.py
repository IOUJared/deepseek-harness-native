"""Pure executable observation checks: no processes, actual /proc or Node invocations."""
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('explicit_node_qualifier_tests', Path(__file__).with_name('qualify-explicit-node.py'))
Q = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(Q)


class NodeImageTests(unittest.TestCase):
    key = (123, 456)
    node = Path('/PUBLIC/node private copy · 世界')
    process = dict(pid=123, start_ticks=456)
    inode = SimpleNamespace(st_mode=0o100700, st_dev=7, st_ino=8)

    def observe(self, stats=None, path=None, inode=None):
        with patch.object(Q.REG.METRICS, 'proc_stat', side_effect=stats or [self.process, self.process]), \
             patch.object(Q.os, 'readlink', return_value=str(self.node) if path is None else path), \
             patch.object(Path, 'stat', side_effect=[self.inode, self.inode if inode is None else inode]):
            return Q.node_image_matches(self.key, self.node)

    def test_matching_identity_path_and_inode_admits_only_point_observation(self):
        self.assertTrue(self.observe())

    def test_same_bytes_or_inode_at_system_path_does_not_prove_selected_path(self):
        self.assertFalse(self.observe(path='/usr/bin/node'))

    def test_distinct_selected_inode_refuses(self):
        self.assertFalse(self.observe(inode=SimpleNamespace(st_mode=0o100700, st_dev=7, st_ino=9)))

    def test_missing_or_reused_identity_before_or_after_refuses(self):
        other = dict(pid=123, start_ticks=457)
        for stats in [[None], [other], [self.process, None], [self.process, other]]:
            self.assertFalse(self.observe(stats=stats))

    def test_proc_executable_error_refuses_without_throwing_private_path(self):
        with patch.object(Q.REG.METRICS, 'proc_stat', return_value=self.process), \
             patch.object(Q.os, 'readlink', side_effect=OSError('PUBLIC private path')):
            self.assertFalse(Q.node_image_matches(self.key, self.node))


if __name__ == '__main__':
    unittest.main()
