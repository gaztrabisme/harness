"""Frozen acceptance oracle — calc-modulo multi-file kata.

Fixed success criterion, written before any agent runs; NEVER edited to fit a
result. An existing 3-file recursive-descent calculator (lexer.py / grammar.py /
evaluator.py) supports  + - * /  parens and unary minus. The task: add the `%`
(modulo) operator at multiplicative precedence, WITHOUT breaking existing ops.

Public entry point:  evaluator.evaluate(expr: str) -> number

REGRESSION tests must stay green (nothing clobbered); FEATURE tests cover `%`.

Run:
    python3 -m unittest discover -s dogfood/calc-modulo -t dogfood/calc-modulo -v
"""

import unittest

from evaluator import evaluate


class TestCalc(unittest.TestCase):
    def test_regression_00(self):
        # evaluate('1') -> 1
        self.assertEqual(evaluate('1'), 1)

    def test_regression_01(self):
        # evaluate('42') -> 42
        self.assertEqual(evaluate('42'), 42)

    def test_regression_02(self):
        # evaluate('3.5') -> 3.5
        self.assertEqual(evaluate('3.5'), 3.5)

    def test_regression_03(self):
        # evaluate('1+2') -> 3
        self.assertEqual(evaluate('1+2'), 3)

    def test_regression_04(self):
        # evaluate('10-4') -> 6
        self.assertEqual(evaluate('10-4'), 6)

    def test_regression_05(self):
        # evaluate('2*3') -> 6
        self.assertEqual(evaluate('2*3'), 6)

    def test_regression_06(self):
        # evaluate('8/2') -> 4.0
        self.assertEqual(evaluate('8/2'), 4.0)

    def test_regression_07(self):
        # evaluate('7/2') -> 3.5
        self.assertEqual(evaluate('7/2'), 3.5)

    def test_regression_08(self):
        # evaluate('2+3*4') -> 14
        self.assertEqual(evaluate('2+3*4'), 14)

    def test_regression_09(self):
        # evaluate('(2+3)*4') -> 20
        self.assertEqual(evaluate('(2+3)*4'), 20)

    def test_regression_10(self):
        # evaluate('2*3+4') -> 10
        self.assertEqual(evaluate('2*3+4'), 10)

    def test_regression_11(self):
        # evaluate('10-2-3') -> 5
        self.assertEqual(evaluate('10-2-3'), 5)

    def test_regression_12(self):
        # evaluate('-5') -> -5
        self.assertEqual(evaluate('-5'), -5)

    def test_regression_13(self):
        # evaluate('-(2+3)') -> -5
        self.assertEqual(evaluate('-(2+3)'), -5)

    def test_regression_14(self):
        # evaluate('2*-3') -> -6
        self.assertEqual(evaluate('2*-3'), -6)

    def test_regression_15(self):
        # evaluate('-2*-3') -> 6
        self.assertEqual(evaluate('-2*-3'), 6)

    def test_regression_16(self):
        # evaluate('((1+2)*(3+4))') -> 21
        self.assertEqual(evaluate('((1+2)*(3+4))'), 21)

    def test_regression_17(self):
        # evaluate('100/4/5') -> 5.0
        self.assertEqual(evaluate('100/4/5'), 5.0)

    def test_regression_18(self):
        # evaluate('1+2+3+4+5') -> 15
        self.assertEqual(evaluate('1+2+3+4+5'), 15)

    def test_regression_19(self):
        # evaluate('3.5*2') -> 7.0
        self.assertEqual(evaluate('3.5*2'), 7.0)

    def test_regression_20(self):
        # evaluate('1.5+1.5') -> 3.0
        self.assertEqual(evaluate('1.5+1.5'), 3.0)

    def test_feature_00(self):
        # evaluate('10%3') -> 1
        self.assertEqual(evaluate('10%3'), 1)

    def test_feature_01(self):
        # evaluate('10%2') -> 0
        self.assertEqual(evaluate('10%2'), 0)

    def test_feature_02(self):
        # evaluate('7%4') -> 3
        self.assertEqual(evaluate('7%4'), 3)

    def test_feature_03(self):
        # evaluate('9%3') -> 0
        self.assertEqual(evaluate('9%3'), 0)

    def test_feature_04(self):
        # evaluate('10%3+1') -> 2
        self.assertEqual(evaluate('10%3+1'), 2)

    def test_feature_05(self):
        # evaluate('2+10%3') -> 3
        self.assertEqual(evaluate('2+10%3'), 3)

    def test_feature_06(self):
        # evaluate('10%3*2') -> 2
        self.assertEqual(evaluate('10%3*2'), 2)

    def test_feature_07(self):
        # evaluate('2*10%3') -> 2
        self.assertEqual(evaluate('2*10%3'), 2)

    def test_feature_08(self):
        # evaluate('(2+3)%4') -> 1
        self.assertEqual(evaluate('(2+3)%4'), 1)

    def test_feature_09(self):
        # evaluate('10%(2+1)') -> 1
        self.assertEqual(evaluate('10%(2+1)'), 1)

    def test_feature_10(self):
        # evaluate('17%5%3') -> 2
        self.assertEqual(evaluate('17%5%3'), 2)

    def test_feature_11(self):
        # evaluate('-10%3') -> 2
        self.assertEqual(evaluate('-10%3'), 2)

    def test_feature_12(self):
        # evaluate('100%7') -> 2
        self.assertEqual(evaluate('100%7'), 2)

    def test_feature_13(self):
        # evaluate('8%3*2+1') -> 5
        self.assertEqual(evaluate('8%3*2+1'), 5)


if __name__ == "__main__":
    unittest.main(verbosity=2)
