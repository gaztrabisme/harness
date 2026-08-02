"""Frozen acceptance oracle for the signature-ripple kata (shakedown #7).

The task: change `pricing.line_total(qty, unit_price)` to a REQUIRED-parameter
form `line_total(qty, unit_price, discount_pct)` returning
`qty * unit_price * (1 - discount_pct / 100)`, then update every caller across
cart.py / invoice.py / report.py to pass the discount documented in the comment
at its call site.

Why this oracle can't be faked:
  * The two `TestSignature` cases force the change to live *in* `line_total`
    (required 3rd parameter + the discount math) — they block both the
    optional-default shortcut and the "hardcode the total in the caller" bypass.
  * The seven value tests catch a missed or lazily-edited caller.
  * `basket_b` and `rush_fee` are 0%-discount tripwires: they already pass on
    `main` (undiscounted == discounted at 0%), so a model that skips them as
    "no change needed" leaves a 2-arg call that crashes once the signature
    becomes 3 required args.

Expected values were generated from plain Python arithmetic (the op set is a
strict subset of Python), and are baked as literals — nothing to import from the
code under test. `assertAlmostEqual` is used throughout for float safety.
"""

import unittest

from pricing import line_total
from cart import basket_a, basket_b
from invoice import invoice_subtotal, rush_fee, bulk_line
from report import featured_total, clearance_total


class TestSignature(unittest.TestCase):
    def test_discount_is_required(self):
        # After the refactor discount_pct is REQUIRED — the old 2-arg call must
        # no longer be accepted (this rejects an optional default).
        with self.assertRaises(TypeError):
            line_total(2, 5.0)

    def test_discount_math_lives_in_line_total(self):
        # 2 * 10.0 * (1 - 50/100) == 10.0 — forces the math through line_total.
        self.assertAlmostEqual(line_total(2, 10.0, 50), 10.0)


class TestCallers(unittest.TestCase):
    def test_basket_a(self):
        self.assertAlmostEqual(basket_a(), 24.0)

    def test_basket_b(self):
        self.assertAlmostEqual(basket_b(), 35.0)

    def test_invoice_subtotal(self):
        self.assertAlmostEqual(invoice_subtotal(), 90.0)

    def test_rush_fee(self):
        self.assertAlmostEqual(rush_fee(), 40.0)

    def test_bulk_line(self):
        self.assertAlmostEqual(bulk_line(), 64.35)

    def test_featured_total(self):
        self.assertAlmostEqual(featured_total(), 50.9745)

    def test_clearance_total(self):
        self.assertAlmostEqual(clearance_total(), 20.0)


if __name__ == "__main__":
    unittest.main()
