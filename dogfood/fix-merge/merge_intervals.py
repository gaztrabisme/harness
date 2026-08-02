"""Merge overlapping and touching intervals.

NOTE: This file ships with a DELIBERATE seeded bug — it is the starting fixture
for the fix-merge harness shakedown (a bug-fix task). The agent's job is to make
test_merge_intervals.py pass. Do not "pre-fix" this on main; the red state is
intentional.
"""


def merge_intervals(intervals):
    if not intervals:
        return []

    ordered = sorted(intervals, key=lambda iv: iv[0])
    merged = [list(ordered[0])]

    for start, end in ordered[1:]:
        prev_start, prev_end = merged[-1]
        # BUG: strict `<` drops the touching case (start == prev_end), so
        # [1,2] and [2,3] are left unmerged. The fix is `<=`.
        if start < prev_end:
            merged[-1][1] = max(prev_end, end)
        else:
            merged.append([start, end])

    return merged
