"""Check numeric text against independent integer and exact Decimal models.

Optional Jackcess rows provide native input, not native expression semantics.
Nothing executes database application content; native Access remains unrun.
"""
import argparse
import csv
from decimal import Decimal, ROUND_HALF_EVEN, localcontext
from io import StringIO
from pathlib import Path
import random
import struct
import subprocess

from compare_browsing import RECIPES, display, run
from compare_large_tables import digest


def spelling(text):
    return display('T'+text.encode('utf-8').hex())


def floating(value, digits):
    if value == 0:
        return ' 0'
    with localcontext() as context:
        context.prec = 400
        exact = Decimal.from_float(abs(value))
        rounded = exact.quantize(Decimal(1).scaleb(exact.adjusted()-digits+1), rounding=ROUND_HALF_EVEN)
        exponent = rounded.adjusted()
        if exponent >= digits:
            mantissa = format(rounded.scaleb(-exponent), 'f').rstrip('0').rstrip('.')
            text = mantissa+f'E+{exponent}'
        else:
            text = format(rounded, 'f')
            if '.' in text:
                text = text.rstrip('0').rstrip('.')
    return ('-' if value < 0 else ' ')+text


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    rng = random.Random(20261020)
    controls = []
    integers = [0, 5, 8, 10, 459, -1, -32767, -32768, -32769,
                -(2**31), -(2**31)-1, 2**31, -(2**63), 2**63-1]
    integers += [rng.randrange(-(2**63), 2**63) for _ in range(512)]
    for value in integers:
        bits = value if value >= 0 else value % (2**(16 if value > -32768 else 32 if value >= -(2**31) else 64))
        controls.extend([(f'Hex({value})', format(bits, 'X')), (f'Oct({value})', format(bits, 'o')),
                         (f'Str({value})', (' ' if value >= 0 else '')+str(value))])
    values = [0.0, -0.0, 459.0, -459.65, 459.001, 0.1, 1e-20, 1e15,
              1.2345678901234567, float.fromhex('0x0.0000000000001p-1022'),
              float.fromhex('0x1.fffffffffffffp+1023')]
    values += [rng.uniform(-1, 1)*10**rng.randrange(-300, 300) for _ in range(256)]
    for value in values:
        controls.append((f'Str(CDbl({value!r}))', floating(value, 15)))
    for value in [0.1, -0.1, 12345678.0, 1e-30, 1e30]:
        single = struct.unpack('f', struct.pack('f', value))[0]
        controls.append((f'Str(CSng({value!r}))', floating(single, 7)))
    count = 0
    for mode in ['eval', 'run']:
        # Expanded subnormal fractions can consume hundreds of charged steps.
        # Two projections fit the ordinary scalar command's work ceiling.
        for start in range(0, len(controls), 2):
            batch = controls[start:start+2]
            sql = 'SELECT '+','.join(f'{expression} AS C{i}' for i,(expression,_) in enumerate(batch))
            result = subprocess.run([str(args.cli), 'query', mode, sql], capture_output=True, check=True, text=True)
            rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
            assert len(rows) == 2 and len(rows[1]) == len(batch)
            for cell,(expression,expected) in zip(rows[1], batch):
                assert cell == spelling(expected), (mode, expression, cell, expected)
                count += 1
        for expression in ["Hex('x')", "Str('1')", 'Hex(CDbl(2147483648))', 'Oct(1,1/0)']:
            result = subprocess.run([str(args.cli), 'query', mode, 'SELECT 1,'+expression], capture_output=True)
            assert result.returncode != 0 and not result.stdout, expression
            count += 1
    if args.jar:
        assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before = digest(args.database)
        tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar,
                     RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
        expected = []
        for line in tagged.splitlines():
            value = int(line.split('\t')[1].removeprefix('I32:'))
            assert value >= 0
            expected.append([str(value), spelling(format(value, 'X')), spelling(format(value, 'o')), spelling(' '+str(value))])
        expected.sort(key=lambda row: int(row[0]))
        sql = 'SELECT ORDR_NBR AS N,Hex(ORDR_NBR) AS H,Oct(ORDR_NBR) AS O,Str(ORDR_NBR) AS S FROM ROULETTE_TB ORDER BY N'
        for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
            result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(args.database), *options], capture_output=True, check=True, text=True)
            actual = list(csv.reader(StringIO(result.stdout), delimiter='\t'))[1:]
            assert actual == expected
            count += len(expected)*3
        assert digest(args.database) == before
    print(f'Passed {count} independent numeric-text/refusal/native-row controls; native Access unrun.')


if __name__ == '__main__':
    main()
