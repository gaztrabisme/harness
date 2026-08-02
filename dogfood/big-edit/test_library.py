"""Frozen acceptance oracle — big-edit (library.py) kata.

Fixed success criterion, written before any agent runs; NEVER edited to fit a
result. library.py is a grab-bag of independent pure functions with exactly one
deliberate bug: is_prime() lacks its `n < 2` guard and reports 0 and 1 as prime.
The task: fix is_prime WITHOUT breaking any other function. is_prime sits in the
file's elided middle, so the fix requires navigating the offload preview (re-read
a slice or grep) rather than blindly overwriting the whole file.

regression_* must stay green (nothing clobbered; fib lives in the elided block and
is the clobber tripwire); isprime_* covers the bug (0 and 1 fail on the baseline).

Run:
    python3 -m unittest discover -s dogfood/big-edit -t dogfood/big-edit -v
"""

import unittest

from library import (
    add, subtract, multiply, power, clamp, sign, abs_val, maximum, minimum,
    average, square, cube, double, halve, mod, floordiv, is_positive,
    is_negative, between, negate, gcd, lcm, factorial, fib, is_prime,
    divisors, is_perfect, reverse, count_vowels, is_palindrome, digit_sum,
    title_case, repeat,
)


class TestLibrary(unittest.TestCase):
    def test_regression_00(self):
        self.assertEqual(add(2, 3), 5)

    def test_regression_01(self):
        self.assertEqual(add(-1, 1), 0)

    def test_regression_02(self):
        self.assertEqual(subtract(10, 4), 6)

    def test_regression_03(self):
        self.assertEqual(multiply(6, 7), 42)

    def test_regression_04(self):
        self.assertEqual(power(2, 10), 1024)

    def test_regression_05(self):
        self.assertEqual(power(5, 0), 1)

    def test_regression_06(self):
        self.assertEqual(clamp(15, 0, 10), 10)

    def test_regression_07(self):
        self.assertEqual(clamp(-3, 0, 10), 0)

    def test_regression_08(self):
        self.assertEqual(clamp(5, 0, 10), 5)

    def test_regression_09(self):
        self.assertEqual(sign(-3), -1)

    def test_regression_10(self):
        self.assertEqual(sign(0), 0)

    def test_regression_11(self):
        self.assertEqual(sign(8), 1)

    def test_regression_12(self):
        self.assertEqual(abs_val(-7), 7)

    def test_regression_13(self):
        self.assertEqual(abs_val(4), 4)

    def test_regression_14(self):
        self.assertEqual(maximum(3, 9), 9)

    def test_regression_15(self):
        self.assertEqual(minimum(3, 9), 3)

    def test_regression_16(self):
        self.assertEqual(average(2, 4), 3.0)

    def test_regression_17(self):
        self.assertEqual(square(5), 25)

    def test_regression_18(self):
        self.assertEqual(cube(3), 27)

    def test_regression_19(self):
        self.assertEqual(double(6), 12)

    def test_regression_20(self):
        self.assertEqual(halve(10), 5.0)

    def test_regression_21(self):
        self.assertEqual(mod(10, 3), 1)

    def test_regression_22(self):
        self.assertEqual(floordiv(17, 5), 3)

    def test_regression_23(self):
        self.assertEqual(is_positive(5), True)

    def test_regression_24(self):
        self.assertEqual(is_positive(-2), False)

    def test_regression_25(self):
        self.assertEqual(is_negative(-2), True)

    def test_regression_26(self):
        self.assertEqual(between(5, 1, 10), True)

    def test_regression_27(self):
        self.assertEqual(between(0, 1, 10), False)

    def test_regression_28(self):
        self.assertEqual(negate(4), -4)

    def test_regression_29(self):
        self.assertEqual(gcd(12, 18), 6)

    def test_regression_30(self):
        self.assertEqual(gcd(17, 5), 1)

    def test_regression_31(self):
        self.assertEqual(lcm(4, 6), 12)

    def test_regression_32(self):
        self.assertEqual(lcm(0, 5), 0)

    def test_regression_33(self):
        self.assertEqual(factorial(5), 120)

    def test_regression_34(self):
        self.assertEqual(factorial(0), 1)

    def test_regression_35(self):
        self.assertEqual(fib(10), 55)

    def test_regression_36(self):
        self.assertEqual(fib(0), 0)

    def test_regression_37(self):
        self.assertEqual(fib(1), 1)

    def test_regression_38(self):
        self.assertEqual(divisors(12), [1, 2, 3, 4, 6, 12])

    def test_regression_39(self):
        self.assertEqual(divisors(7), [1, 7])

    def test_regression_40(self):
        self.assertEqual(is_perfect(6), True)

    def test_regression_41(self):
        self.assertEqual(is_perfect(28), True)

    def test_regression_42(self):
        self.assertEqual(is_perfect(12), False)

    def test_regression_43(self):
        self.assertEqual(reverse('abc'), 'cba')

    def test_regression_44(self):
        self.assertEqual(count_vowels('Hello World'), 3)

    def test_regression_45(self):
        self.assertEqual(is_palindrome('racecar'), True)

    def test_regression_46(self):
        self.assertEqual(is_palindrome('abc'), False)

    def test_regression_47(self):
        self.assertEqual(digit_sum(12345), 15)

    def test_regression_48(self):
        self.assertEqual(title_case('hello world'), 'Hello World')

    def test_regression_49(self):
        self.assertEqual(repeat('ab', 3), 'ababab')

    def test_isprime_00(self):
        self.assertEqual(is_prime(0), False)

    def test_isprime_01(self):
        self.assertEqual(is_prime(1), False)

    def test_isprime_02(self):
        self.assertEqual(is_prime(2), True)

    def test_isprime_03(self):
        self.assertEqual(is_prime(3), True)

    def test_isprime_04(self):
        self.assertEqual(is_prime(4), False)

    def test_isprime_05(self):
        self.assertEqual(is_prime(17), True)

    def test_isprime_06(self):
        self.assertEqual(is_prime(20), False)

    def test_isprime_07(self):
        self.assertEqual(is_prime(97), True)


if __name__ == "__main__":
    unittest.main(verbosity=2)
