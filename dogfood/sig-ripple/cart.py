"""Shopping-cart subtotals. Each function prices one basket via `line_total`."""

from pricing import line_total


def basket_a():
    # 2 units @ $15.00, 20% off
    return line_total(2, 15.0)


def basket_b():
    # 10 units @ $3.50, no discount (0%)
    return line_total(10, 3.5)
