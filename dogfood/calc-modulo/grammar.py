"""Recursive-descent parser for the calculator.

Consumes the token list from `lexer.tokenize` and builds a small AST of tuples:

    ("num", value)              numeric literal
    ("neg", child)              unary minus
    ("bin", op, left, right)    binary operator

Grammar (lowest precedence first):

    expr   = term  (("+" | "-") term)*
    term   = unary (("*" | "/") unary)*      # multiplicative, left-associative
    unary  = "-" unary | atom
    atom   = NUM | "(" expr ")"

The evaluator gives the operators their arithmetic meaning; this layer only
fixes precedence and associativity.
"""

from lexer import tokenize


class _Parser:
    def __init__(self, tokens):
        self.tokens = tokens
        self.pos = 0

    def _peek(self):
        return self.tokens[self.pos] if self.pos < len(self.tokens) else None

    def _eat(self):
        tok = self.tokens[self.pos]
        self.pos += 1
        return tok

    def _is_op(self, *ops):
        tok = self._peek()
        return tok is not None and tok[0] == "OP" and tok[1] in ops

    def parse(self):
        node = self.expr()
        if self.pos != len(self.tokens):
            raise SyntaxError("trailing tokens after expression")
        return node

    def expr(self):
        node = self.term()
        while self._is_op("+", "-"):
            op = self._eat()[1]
            node = ("bin", op, node, self.term())
        return node

    def term(self):
        node = self.unary()
        while self._is_op("*", "/"):
            op = self._eat()[1]
            node = ("bin", op, node, self.unary())
        return node

    def unary(self):
        if self._is_op("-"):
            self._eat()
            return ("neg", self.unary())
        return self.atom()

    def atom(self):
        tok = self._peek()
        if tok is None:
            raise SyntaxError("unexpected end of input")
        if tok[0] == "NUM":
            self._eat()
            return ("num", tok[1])
        if self._is_op("("):
            self._eat()
            node = self.expr()
            if not self._is_op(")"):
                raise SyntaxError("expected closing paren")
            self._eat()
            return node
        raise SyntaxError(f"unexpected token {tok!r}")


def parse(src):
    """Tokenize `src` and return its AST."""
    return _Parser(tokenize(src)).parse()
