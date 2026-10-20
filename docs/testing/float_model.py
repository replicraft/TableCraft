"""Independent bounded exact-rational IEEE expectations for getter comparisons.

Binary32 uses a search over its finite positive encodings, then exact rational
neighbor distances/ties-even. It never rounds through binary64 first. Binary64
uses Python's exact Fraction-to-float conversion. This is engineering test code,
not native Access evidence or a production numeric dependency.
"""
from fractions import Fraction
import struct

F32_MAX_BITS = 0x7F7FFFFF
F32_SIGN_BIT = 0x80000000


def _rational(bits):
    return Fraction(struct.unpack(">f", struct.pack(">I", bits))[0])


def expected_bits(value, single, negative_zero=False):
    value = Fraction(value)
    negative = value < 0 or (value == 0 and negative_zero)
    if not single:
        result = float(value)
        if negative and value == 0:
            result = -0.0
        return struct.pack(">d", result)
    magnitude = abs(value)
    if magnitude > _rational(F32_MAX_BITS):
        raise OverflowError("Outside finite binary32 range")
    low, high = 0, F32_MAX_BITS
    while low < high:
        middle = (low + high + 1) // 2
        if _rational(middle) <= magnitude:
            low = middle
        else:
            high = middle - 1
    bits = low
    lower = _rational(low)
    if lower != magnitude:
        upper = _rational(low + 1)
        if upper - magnitude < magnitude - lower or (upper - magnitude == magnitude - lower and low & 1):
            bits += 1
    return struct.pack(">I", bits | (F32_SIGN_BIT if negative else 0))


def display_bits(cell, single):
    """Round-trip a shortest CLI float display to the output carrier's bits."""
    return struct.pack(">f" if single else ">d", float(cell))
