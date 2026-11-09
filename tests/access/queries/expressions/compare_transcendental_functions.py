"""Compare Double math to high-precision Decimal models, without Access claims.

The trig reference uses convergent power series and Machin's identity for pi;
it does not call the platform sine/cosine/tangent/arctangent implementation.
"""
import argparse
from decimal import Decimal, localcontext
import math
from pathlib import Path
import subprocess

from compare_browsing import run

REFERENCE_PRECISION = 100
SERIES_TERMS = 2048
SERIES_TOLERANCE = Decimal('1e-95')
MAX_ULPS = 4


def atan_series(x):
    if x < 0:
        return -atan_series(-x)
    term, result = x, x
    for n in range(1, SERIES_TERMS):
        term *= -x * x
        change = term / (2 * n + 1)
        result += change
        if abs(change) < SERIES_TOLERANCE:
            return result
    raise ArithmeticError('Reference series failed to converge')


def trig(x, cosine=False):
    term = Decimal(1) if cosine else x
    result = term
    for n in range(1, SERIES_TERMS):
        a = 2 * n - (1 if cosine else 0)
        term *= -x * x / (a * (a + 1))
        result += term
        if abs(term) < SERIES_TOLERANCE:
            return result
    raise ArithmeticError('Reference series failed to converge')


def reference(name, value):
    with localcontext() as context:
        context.prec = REFERENCE_PRECISION
        x = Decimal.from_float(float(value))
        if name == 'Sqr':
            return float(x.sqrt())
        if name == 'Exp':
            return float(x.exp())
        if name == 'Log':
            return float(x.ln())
        pi = 16 * atan_series(Decimal(1) / 5) - 4 * atan_series(Decimal(1) / 239)
        if name == 'Atn':
            sign = -1 if x < 0 else 1
            x = abs(x)
            result = pi / 4 if x == 1 else (pi / 2 - atan_series(1 / x) if x > 1 else atan_series(x))
            return float(sign * result)
        # Corpus angles are small; exact pi reduction avoids a shared libm path.
        x %= 2 * pi
        if name == 'Sin':
            return float(trig(x))
        if name == 'Cos':
            return float(trig(x, True))
        if name == 'Tan':
            return float(trig(x) / trig(x, True))
        raise ValueError('Unknown reference function')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    args = parser.parse_args()
    cases = {
        'Sqr': ['0', '0.25', '1', '2', '4', '23', '1e-20', '1e20'],
        'Exp': ['-1000', '-700', '-10', '-1', '0', '0.5', '1', '10', '700', '709'],
        'Log': ['1e-20', '0.25', '0.5', '1', '2', '10', '1e20'],
        'Sin': ['-6', '-1', '-0.25', '0', '0.25', '1', '6'],
        'Cos': ['-6', '-1', '-0.25', '0', '0.25', '1', '6'],
        'Tan': ['-6', '-1', '-0.25', '0', '0.25', '1', '6'],
        'Atn': ['-2', '-1', '-0.75', '0', '0.5', '1', '2'],
    }
    controls = 0
    for mode in ['eval', 'run']:
        for name, values in cases.items():
            for value in values:
                output = run(args.cli, 'query', mode, f'SELECT {name}(CDbl({value})) AS N')
                header, cell = output.splitlines()
                assert header == 'N'
                expected, actual = reference(name, value), float(cell)
                assert math.isfinite(actual)
                assert abs(actual - expected) <= MAX_ULPS * math.ulp(expected), (mode, name, value, actual, expected)
                controls += 1
            assert run(args.cli, 'query', mode, f'SELECT {name}(NULL) AS N') == 'N\nNULL\n'
            controls += 1
        for expression in ['Sqr(-1)', 'Log(0)', 'Log(-1)', 'Exp(710)', 'Sin(1,2)', 'IIf(False,Sqr(-1),0)', 'Sin(1/0)']:
            result = subprocess.run([str(args.cli), 'query', mode, f'SELECT 1 AS Before,{expression} AS N'], capture_output=True)
            assert result.returncode != 0 and result.stdout == b'', (mode, expression)
            controls += 1
    print(f'Passed {controls} high-precision/NULL/refusal math controls; tolerance {MAX_ULPS} binary64 ULPs; native Access unrun.')


if __name__ == '__main__':
    main()
