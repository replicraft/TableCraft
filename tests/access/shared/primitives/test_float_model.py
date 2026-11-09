"""Independent IEEE goldens for the engineering comparison oracle itself."""
from fractions import Fraction
import unittest
from float_model import expected_bits


class FloatModelTests(unittest.TestCase):
    def test_single_neighbors_ties_and_signed_subnormals(self):
        midpoint=Fraction(1)+Fraction(1,2**24)
        self.assertEqual(expected_bits(midpoint,True).hex(),"3f800000")
        self.assertEqual(expected_bits(midpoint+Fraction(1,10**27),True).hex(),"3f800001")
        self.assertEqual(expected_bits(-midpoint-Fraction(1,10**27),True).hex(),"bf800001")
        self.assertEqual(expected_bits(Fraction(1,2**150),True).hex(),"00000000")
        self.assertEqual(expected_bits(Fraction(3,2**150),True).hex(),"00000002")
        self.assertEqual(expected_bits(0,True,True).hex(),"80000000")

    def test_exact_currency_to_double_and_int64_rounding(self):
        self.assertEqual(expected_bits(Fraction(6811109015807608224,10000),False).hex(),"43035bbbeabb1ec7")
        self.assertEqual(expected_bits(9007199254740993,False).hex(),"4340000000000000")
