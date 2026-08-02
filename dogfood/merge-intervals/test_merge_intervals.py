"""Acceptance oracle for the merge_intervals shakedown kata (run #2).

Fixed success criterion — written before the agent runs, never edited to fit the
result. The agent writes `merge_intervals.py` (beside this file) exporting:

    merge_intervals(intervals: list[list[int]]) -> list[list[int]]

Merge all overlapping AND touching intervals; return them sorted ascending by
start. Run:

    python3 -m unittest discover -s dogfood/merge-intervals -t dogfood/merge-intervals -v

The discriminating pair is `test_touching_merges` vs `test_gap_stays_split`:
touching endpoints merge ([1,2]+[2,3] -> [1,3]) but a real gap does not
([1,2], [4,5] stay separate). That boundary is the classic <= vs < bug.
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
        # Adjacent endpoints MUST merge: [1,2] and [2,3] -> [1,3].
        self.assertEqual(merge_intervals([[1, 2], [2, 3]]), [[1, 3]])

    def test_gap_stays_split(self):
        # A real gap MUST NOT merge: [1,2] and [4,5] are disjoint.
        self.assertEqual(merge_intervals([[1, 2], [4, 5]]), [[1, 2], [4, 5]])

    def test_nested(self):
        # A fully-contained interval is absorbed by the outer one.
        self.assertEqual(merge_intervals([[1, 10], [2, 5]]), [[1, 10]])

    def test_unsorted_input(self):
        # Input order is arbitrary; output is sorted by start.
        self.assertEqual(merge_intervals([[2, 4], [1, 3]]), [[1, 4]])

    def test_cascade_chain(self):
        # Several merges, with a clean break in the middle.
        self.assertEqual(
            merge_intervals([[1, 4], [2, 5], [7, 9], [8, 10]]),
            [[1, 5], [7, 10]],
        )

    def test_does_not_mutate_input(self):
        # The function must not reorder or alter the caller's list in place.
        data = [[3, 4], [1, 2]]
        snapshot = [list(x) for x in data]
        merge_intervals(data)
        self.assertEqual(data, snapshot)


if __name__ == "__main__":
    unittest.main(verbosity=2)
