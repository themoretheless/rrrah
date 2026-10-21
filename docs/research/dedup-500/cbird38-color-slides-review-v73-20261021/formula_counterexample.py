"""Independent arithmetic check of the formula printed on slide 25.

This does not execute cbird or a reference implementation. It intentionally
preserves the printed intersection-only numerator to test a proposed port.
"""
from fractions import Fraction


def printed_similarity(query, target):
    common = query.keys() & target.keys()
    numerator = sum(abs(query[key] - target[key]) for key in common)
    denominator = sum(query.values()) + sum(target.values())
    return 1 - Fraction(numerator, denominator)


same = printed_similarity({1: 1}, {1: 1})
disjoint = printed_similarity({1: 1}, {2: 1})
assert same == 1
assert disjoint == 1
assert {1: 1} != {2: 1}
print('Identical descriptors: similarity =', same)
print('Disjoint nonempty descriptors: similarity =', disjoint)
print('Printed formula alone cannot distinguish these cases; no port accepted.')
