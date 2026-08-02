"""Reporting totals. Each function prices one featured line via `line_total`."""

from pricing import line_total


def featured_total():
    # 3 @ $19.99, 15% off
    return line_total(3, 19.99)


def clearance_total():
    # 8 @ $5.00, 50% off
    return line_total(8, 5.0)
