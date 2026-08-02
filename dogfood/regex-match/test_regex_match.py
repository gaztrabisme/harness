"""Frozen acceptance oracle — regex-match kata (explore-fanout stress run).

Fixed success criterion, written before any worker runs; NEVER edited to fit a
result. The agent writes `regex_match.py` (beside this file) exporting:

    is_match(s: str, p: str) -> bool

Return True iff pattern `p` matches the ENTIRE string `s` (not a substring).
Supported pattern syntax:
    .   matches any single character
    *   matches zero or more of the PRECEDING element
    +   matches one or more of the PRECEDING element
    ?   matches zero or one of the PRECEDING element
    any other char matches itself literally
Quantifiers are greedy with backtracking, exactly like standard regex.

Run:
    python3 -m unittest discover -s dogfood/regex-match -t dogfood/regex-match -v
"""

import unittest

from regex_match import is_match


class TestRegexMatch(unittest.TestCase):
    def test_case_00(self):
        # is_match('', '') -> True
        self.assertEqual(is_match('', ''), True)

    def test_case_01(self):
        # is_match('a', 'a') -> True
        self.assertEqual(is_match('a', 'a'), True)

    def test_case_02(self):
        # is_match('abc', 'abc') -> True
        self.assertEqual(is_match('abc', 'abc'), True)

    def test_case_03(self):
        # is_match('abc', 'abd') -> False
        self.assertEqual(is_match('abc', 'abd'), False)

    def test_case_04(self):
        # is_match('a', '') -> False
        self.assertEqual(is_match('a', ''), False)

    def test_case_05(self):
        # is_match('', 'a') -> False
        self.assertEqual(is_match('', 'a'), False)

    def test_case_06(self):
        # is_match('ab', 'abc') -> False
        self.assertEqual(is_match('ab', 'abc'), False)

    def test_case_07(self):
        # is_match('abc', 'ab') -> False
        self.assertEqual(is_match('abc', 'ab'), False)

    def test_case_08(self):
        # is_match('a', '.') -> True
        self.assertEqual(is_match('a', '.'), True)

    def test_case_09(self):
        # is_match('', '.') -> False
        self.assertEqual(is_match('', '.'), False)

    def test_case_10(self):
        # is_match('abc', 'a.c') -> True
        self.assertEqual(is_match('abc', 'a.c'), True)

    def test_case_11(self):
        # is_match('abc', '...') -> True
        self.assertEqual(is_match('abc', '...'), True)

    def test_case_12(self):
        # is_match('ab', '...') -> False
        self.assertEqual(is_match('ab', '...'), False)

    def test_case_13(self):
        # is_match('abc', '..') -> False
        self.assertEqual(is_match('abc', '..'), False)

    def test_case_14(self):
        # is_match('', 'a*') -> True
        self.assertEqual(is_match('', 'a*'), True)

    def test_case_15(self):
        # is_match('a', 'a*') -> True
        self.assertEqual(is_match('a', 'a*'), True)

    def test_case_16(self):
        # is_match('aaaa', 'a*') -> True
        self.assertEqual(is_match('aaaa', 'a*'), True)

    def test_case_17(self):
        # is_match('aa', 'a') -> False
        self.assertEqual(is_match('aa', 'a'), False)

    def test_case_18(self):
        # is_match('', '.*') -> True
        self.assertEqual(is_match('', '.*'), True)

    def test_case_19(self):
        # is_match('abc', '.*') -> True
        self.assertEqual(is_match('abc', '.*'), True)

    def test_case_20(self):
        # is_match('aab', 'c*a*b') -> True
        self.assertEqual(is_match('aab', 'c*a*b'), True)

    def test_case_21(self):
        # is_match('mississippi', 'mis*is*p*.') -> False
        self.assertEqual(is_match('mississippi', 'mis*is*p*.'), False)

    def test_case_22(self):
        # is_match('mississippi', 'mis*is*ip*.') -> True
        self.assertEqual(is_match('mississippi', 'mis*is*ip*.'), True)

    def test_case_23(self):
        # is_match('aaa', 'a*a') -> True
        self.assertEqual(is_match('aaa', 'a*a'), True)

    def test_case_24(self):
        # is_match('aaa', 'ab*a*c*a') -> True
        self.assertEqual(is_match('aaa', 'ab*a*c*a'), True)

    def test_case_25(self):
        # is_match('abcd', 'd*') -> False
        self.assertEqual(is_match('abcd', 'd*'), False)

    def test_case_26(self):
        # is_match('', 'a+') -> False
        self.assertEqual(is_match('', 'a+'), False)

    def test_case_27(self):
        # is_match('a', 'a+') -> True
        self.assertEqual(is_match('a', 'a+'), True)

    def test_case_28(self):
        # is_match('aaa', 'a+') -> True
        self.assertEqual(is_match('aaa', 'a+'), True)

    def test_case_29(self):
        # is_match('b', 'a+') -> False
        self.assertEqual(is_match('b', 'a+'), False)

    def test_case_30(self):
        # is_match('aab', 'a+b') -> True
        self.assertEqual(is_match('aab', 'a+b'), True)

    def test_case_31(self):
        # is_match('ab', 'a+b') -> True
        self.assertEqual(is_match('ab', 'a+b'), True)

    def test_case_32(self):
        # is_match('b', 'a+b') -> False
        self.assertEqual(is_match('b', 'a+b'), False)

    def test_case_33(self):
        # is_match('abc', '.+') -> True
        self.assertEqual(is_match('abc', '.+'), True)

    def test_case_34(self):
        # is_match('', '.+') -> False
        self.assertEqual(is_match('', '.+'), False)

    def test_case_35(self):
        # is_match('', 'a?') -> True
        self.assertEqual(is_match('', 'a?'), True)

    def test_case_36(self):
        # is_match('a', 'a?') -> True
        self.assertEqual(is_match('a', 'a?'), True)

    def test_case_37(self):
        # is_match('aa', 'a?') -> False
        self.assertEqual(is_match('aa', 'a?'), False)

    def test_case_38(self):
        # is_match('abc', 'ab?c') -> True
        self.assertEqual(is_match('abc', 'ab?c'), True)

    def test_case_39(self):
        # is_match('ac', 'ab?c') -> True
        self.assertEqual(is_match('ac', 'ab?c'), True)

    def test_case_40(self):
        # is_match('abbc', 'ab?c') -> False
        self.assertEqual(is_match('abbc', 'ab?c'), False)

    def test_case_41(self):
        # is_match('aaba', 'a*b.*') -> True
        self.assertEqual(is_match('aaba', 'a*b.*'), True)

    def test_case_42(self):
        # is_match('xaby', '.*ab.*') -> True
        self.assertEqual(is_match('xaby', '.*ab.*'), True)

    def test_case_43(self):
        # is_match('aaa', 'a*a*a*') -> True
        self.assertEqual(is_match('aaa', 'a*a*a*'), True)

    def test_case_44(self):
        # is_match('aaab', 'a*a*a*') -> False
        self.assertEqual(is_match('aaab', 'a*a*a*'), False)

    def test_case_45(self):
        # is_match('aaaaaab', 'a+a+a+b') -> True
        self.assertEqual(is_match('aaaaaab', 'a+a+a+b'), True)

    def test_case_46(self):
        # is_match('ab', 'a.?b') -> True
        self.assertEqual(is_match('ab', 'a.?b'), True)

    def test_case_47(self):
        # is_match('aXb', 'a.?b') -> True
        self.assertEqual(is_match('aXb', 'a.?b'), True)

    def test_case_48(self):
        # is_match('aXYb', 'a.?b') -> False
        self.assertEqual(is_match('aXYb', 'a.?b'), False)


if __name__ == "__main__":
    unittest.main(verbosity=2)
