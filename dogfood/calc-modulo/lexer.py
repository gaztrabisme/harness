"""Lexer for the tiny arithmetic calculator.

Turns a source string into a flat list of tokens. A token is a (kind, value)
pair: ("NUM", 3.0) | ("NUM", 3) for literals, or ("OP", "+") for an operator or
paren. Whitespace is skipped. Unknown characters raise SyntaxError.

Supported operators live in OPERATORS — the single source of truth the rest of
the lexer keys off. The grammar and evaluator layer their own meaning on top.
"""

# Single-character operators and parens this calculator understands.
OPERATORS = set("+-*/()")


def tokenize(src):
    """Return the list of (kind, value) tokens for `src`."""
    tokens = []
    i = 0
    n = len(src)
    while i < n:
        c = src[i]
        if c.isspace():
            i += 1
            continue
        if c.isdigit() or c == ".":
            j = i
            while j < n and (src[j].isdigit() or src[j] == "."):
                j += 1
            text = src[i:j]
            value = float(text) if "." in text else int(text)
            tokens.append(("NUM", value))
            i = j
            continue
        if c in OPERATORS:
            tokens.append(("OP", c))
            i += 1
            continue
        raise SyntaxError(f"unexpected character {c!r} at position {i}")
    return tokens
