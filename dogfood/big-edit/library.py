"""A small grab-bag utility library.

A flat collection of independent pure functions — arithmetic, number theory,
and string helpers. Each function stands alone; there are no cross-dependencies
between them. This module is intentionally long so that a tool that previews
only the head and tail of a file cannot see the middle without re-reading.

Exactly one function below has a known bug (see the project task); every other
function is correct and must keep working unchanged.
"""


# ---------------------------------------------------------------------------
# Basic arithmetic
# ---------------------------------------------------------------------------

def add(a, b):
    """Return a + b."""
    return a + b


def subtract(a, b):
    """Return a - b."""
    return a - b


def multiply(a, b):
    """Return a * b."""
    return a * b


def power(base, exp):
    """Return base raised to the exp power (non-negative integer exp)."""
    result = 1
    for _ in range(exp):
        result *= base
    return result


def clamp(x, lo, hi):
    """Clamp x into the inclusive range [lo, hi]."""
    return max(lo, min(x, hi))


def sign(x):
    """Return -1, 0, or 1 according to the sign of x."""
    if x > 0:
        return 1
    if x < 0:
        return -1
    return 0


def abs_val(x):
    """Return the absolute value of x."""
    return x if x >= 0 else -x


def maximum(a, b):
    """Return the larger of a and b."""
    return a if a >= b else b


def minimum(a, b):
    """Return the smaller of a and b."""
    return a if a <= b else b


# ---------------------------------------------------------------------------
# More arithmetic helpers
# ---------------------------------------------------------------------------

def average(a, b):
    """Return the arithmetic mean of a and b."""
    return (a + b) / 2


def square(x):
    """Return x squared."""
    return x * x


def cube(x):
    """Return x cubed."""
    return x * x * x


def double(x):
    """Return x doubled."""
    return x * 2


def halve(x):
    """Return x divided by two (true division)."""
    return x / 2


def mod(a, b):
    """Return the remainder of a divided by b."""
    return a % b


def floordiv(a, b):
    """Return the floor division of a by b."""
    return a // b


def is_positive(x):
    """Return True iff x is strictly greater than zero."""
    return x > 0


def is_negative(x):
    """Return True iff x is strictly less than zero."""
    return x < 0


def between(x, lo, hi):
    """Return True iff lo <= x <= hi."""
    return lo <= x <= hi


def negate(x):
    """Return the additive inverse of x."""
    return -x


# ---------------------------------------------------------------------------
# Number theory
# ---------------------------------------------------------------------------

def gcd(a, b):
    """Return the greatest common divisor of a and b (Euclid's algorithm)."""
    a, b = abs(a), abs(b)
    while b:
        a, b = b, a % b
    return a


def lcm(a, b):
    """Return the least common multiple of a and b."""
    if a == 0 or b == 0:
        return 0
    return abs(a * b) // gcd(a, b)


def factorial(n):
    """Return n! for a non-negative integer n."""
    result = 1
    for k in range(2, n + 1):
        result *= k
    return result


def fib(n):
    """Return the n-th Fibonacci number (0-indexed: fib(0)=0, fib(1)=1)."""
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a


def is_prime(n):
    """Return True iff n is a prime number.

    BUG: this currently reports 0 and 1 as prime. A prime must be an integer
    greater than or equal to 2 with no positive divisors other than 1 and
    itself. The guard for n < 2 is missing below.
    """
    for d in range(2, int(n ** 0.5) + 1):
        if n % d == 0:
            return False
    return True


def divisors(n):
    """Return the sorted list of positive divisors of a positive integer n."""
    out = []
    for d in range(1, n + 1):
        if n % d == 0:
            out.append(d)
    return out


def is_perfect(n):
    """Return True iff n equals the sum of its proper divisors (e.g. 6, 28)."""
    if n < 2:
        return False
    return sum(d for d in divisors(n) if d != n) == n


# ---------------------------------------------------------------------------
# String helpers
# ---------------------------------------------------------------------------

def reverse(s):
    """Return the string s reversed."""
    return s[::-1]


def count_vowels(s):
    """Return the number of vowels (aeiou, case-insensitive) in s."""
    return sum(1 for c in s.lower() if c in "aeiou")


def is_palindrome(s):
    """Return True iff s reads the same forwards and backwards."""
    return s == s[::-1]


def digit_sum(n):
    """Return the sum of the decimal digits of a non-negative integer n."""
    total = 0
    while n > 0:
        total += n % 10
        n //= 10
    return total


def title_case(s):
    """Return s with the first letter of each space-separated word capitalised."""
    return " ".join(w[:1].upper() + w[1:] for w in s.split(" "))


def repeat(s, n):
    """Return s concatenated n times."""
    return s * n
