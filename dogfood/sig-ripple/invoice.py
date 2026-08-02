"""Invoice line items. Each function prices one line via `line_total`."""

from pricing import line_total


def invoice_subtotal():
    # 4 @ $25.00, 10% off
    return line_total(4, 25.0)


def rush_fee():
    # 1 @ $40.00, no discount (0%)
    return line_total(1, 40.0)


def bulk_line():
    # 100 @ $0.99, 35% off
    return line_total(100, 0.99)
