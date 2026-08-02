"""Evaluator for the calculator AST.

`evaluate(expr)` is the public entry point: it parses `expr` into an AST (via
`grammar.parse`, which in turn calls `lexer.tokenize`) and walks it, returning a
Python number. Operator meaning lives here and nowhere else.
"""

from grammar import parse


def _eval(node):
    kind = node[0]
    if kind == "num":
        return node[1]
    if kind == "neg":
        return -_eval(node[1])
    if kind == "bin":
        op, left, right = node[1], node[2], node[3]
        a = _eval(left)
        b = _eval(right)
        if op == "+":
            return a + b
        if op == "-":
            return a - b
        if op == "*":
            return a * b
        if op == "/":
            return a / b
        raise ValueError(f"unknown operator {op!r}")
    raise ValueError(f"unknown node {node!r}")


def evaluate(expr):
    """Parse and evaluate an arithmetic expression string to a number."""
    return _eval(parse(expr))
