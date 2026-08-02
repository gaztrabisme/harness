"""Core pricing primitives.

`line_total` is the ONE place a line item's money is computed; every caller in
the package routes through it. That is the whole point: changing how a line is
priced should be a single-helper edit whose new signature then ripples out to
every call site.
"""


def line_total(qty, unit_price):
    """Money for a single line: quantity times unit price.

    TODO(pricing-v2): this ignores per-line discounts. The refactor adds a
    REQUIRED ``discount_pct`` parameter (a percentage in 0..100) and returns
    ``qty * unit_price * (1 - discount_pct / 100)``. Because the parameter is
    required (no default), every caller must be updated to pass its discount.
    """
    return qty * unit_price
