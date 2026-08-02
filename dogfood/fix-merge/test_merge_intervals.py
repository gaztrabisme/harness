"""Acceptance oracle for the fix-merge shakedown (run #3 — recovery probe).

Identical 9-case oracle to dogfood/merge-intervals, paired with a deliberately
BUGGY merge_intervals.py in this directory. The ticket is a bug-fix: the agent
must make this suite green. The seeded bug fails at least test_touching_merges,
so the FIRST validation is red by construction — that's the point, it forces the
loop's diagnose -> edit -> re-run recovery path.

    python3 -m unittest discover -s dogfood/fix-merge -t dogfood/fix-merge -v
"""

import unittest

from merge_intervals import merge_intervals


class TestMergeIntervals(unittest.TestCase):
    def test_empty(self):
        self.assertEqual(merge_intervals([]), [])

    def test_single(self):
        self.assertEqual(merge_intervals([[1, 5]]), [[1, 5]])

    def test_overlapping_pair(self):
        self.assertEqual(merge_intervals([[1, 3], [2, 4]]), [[1, 4]])

    def test_touching_merges(self):
        self.assertEqual(merge_intervals([[1, 2], [2, 3]]), [[1, 3]])

    def test_gap_stays_split(self):
        self.assertEqual(merge_intervals([[1, 2], [4, 5]]), [[1, 2], [4, 5]])

    def test_nested(self):
        self.assertEqual(merge_intervals([[1, 10], [2, 5]]), [[1, 10]])

    def test_unsorted_input(self):
        self.assertEqual(merge_intervals([[2, 4], [1, 3]]), [[1, 4]])

    def test_cascade_chain(self):
        self.assertEqual(
            merge_intervals([[1, 4], [2, 5], [7, 9], [8, 10]]),
            [[1, 5], [7, 10]],
        )

    def test_does_not_mutate_input(self):
        data = [[3, 4], [1, 2]]
        snapshot = [list(x) for x in data]
        merge_intervals(data)
        self.assertEqual(data, snapshot)


if __name__ == "__main__":
    unittest.main(verbosity=2)
