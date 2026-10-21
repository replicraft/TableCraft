"""Independent high-precision cashflow certificates and inert native Rate metadata."""
import argparse
import csv
from decimal import Decimal, localcontext
import hashlib
from io import StringIO
import math
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES
from compare_saved_forests import PROFILES

RATE_ABSOLUTE_TOLERANCE = 1e-9
CASHFLOW_RELATIVE_TOLERANCE = Decimal('1e-10')


def factors(rate, periods, begin):
    growth, accumulation = Decimal(1), Decimal(0)
    for _ in range(periods):
        growth *= 1 + rate
        if begin:
            accumulation += 1
        accumulation *= 1 + rate
        if not begin:
            accumulation += 1
    return growth, accumulation


def root(periods, payment, present, future, begin):
    with localcontext() as context:
        context.prec = 120
        low, high = Decimal('-.5'), Decimal('.5')
        for _ in range(300):
            middle = (low + high) / 2
            growth, accumulation = factors(middle, periods, begin)
            residual = present * growth + payment * accumulation + future
            if residual > 0:
                high = middle
            else:
                low = middle
        return float((low + high) / 2)


def verify(output, expected):
    rows = list(csv.reader(StringIO(output), delimiter='\t', strict=True))
    assert rows[0] == expected[0] and len(rows) == len(expected), (rows, expected)
    for actual, wanted in zip(rows[1:], expected[1:]):
        assert len(actual) == len(wanted)
        for cell, value in zip(actual, wanted):
            if value is None:
                assert cell == 'NULL'
            else:
                assert math.isfinite(float(cell)) and abs(float(cell) - float(value)) <= RATE_ABSOLUTE_TOLERANCE, (cell, value)
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('jar', 'cli', 'directory'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--generate', action='store_true')
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    controls = 0

    def cli(*values, accepted=True):
        result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True, text=True, encoding='utf-8')
        assert result.returncode == (0 if accepted else 2), (values, result.stderr)
        if not accepted:
            assert not result.stdout
        return result.stdout

    with localcontext() as context:
        context.prec = 120
        for rate in ['-.05', '0', '.01', '.1']:
            for periods in [2, 12, 60, 360]:
                for begin in [0, 1]:
                    growth, accumulation = factors(Decimal(rate), periods, begin)
                    payment = -Decimal(1000) * growth / accumulation
                    sql = f"SELECT Rate({periods},Pmt({rate},{periods},1000,,{begin}),1000,,{begin},) AS R,Pmt({rate},{periods},1000,,{begin}) AS P"
                    for command in ['eval', 'run']:
                        rows = verify(cli('query', command, sql), [['R', 'P'], [float(rate), float(payment)]])
                        actual_rate, actual_payment = map(Decimal, rows[1])
                        growth, accumulation = factors(actual_rate, periods, begin)
                        residual = Decimal(1000) * growth + actual_payment * accumulation
                        scale = abs(Decimal(1000) * growth) + abs(actual_payment * accumulation)
                        assert abs(residual) <= scale * CASHFLOW_RELATIVE_TOLERANCE, (sql, rows)
                        controls += 1
    for guess, expected in [('.05', .1), ('.25', .2)]:
        for command in ['eval', 'run']:
            verify(cli('query', command, f'SELECT Rate(2,-2.3,1,3.62,0,{guess}) AS R'), [['R'], [expected]])
            controls += 1
    for command in ['eval', 'run']:
        for sql in ['SELECT 1,Rate(12,-100,1000,,,1e100)', 'SELECT 1,Rate(12,100,1000)', 'SELECT 1,Rate(Null,-100,1000,,,1/0)']:
            cli('query', command, sql, accepted=False)
            controls += 1
    with tempfile.TemporaryDirectory(prefix='tablecraft-rate-oracle-') as classes:
        names = ['SavedForestFixture', 'SavedStatisticsFixture', 'SavedRateFixture', 'SavedReadOracle', 'SavedQueryOracle', 'TableDataOracle']
        subprocess.run(['javac', '-encoding', 'UTF-8', '-cp', str(args.jar), '-d', classes,
                        *[str(RECIPES / (name + '.java')) for name in names]], check=True)

        def java(name, *values):
            return subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar) + os.pathsep + classes,
                                           name, *map(str, values)], text=True, encoding='utf-8')

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java('SavedRateFixture', args.directory / ('rate-' + suffix), profile)
        ordinary = root(12, Decimal(-100), Decimal(1000), Decimal(0), False)
        for _, suffix in PROFILES:
            path = args.directory / ('rate-' + suffix)
            original = path.read_bytes()
            values = [int(row.split('\t')[1].removeprefix('I32:')) for row in java('TableDataOracle', path, 'AData').splitlines()]
            assert values == [1, 2, 4]
            expected = {
                'RateAll': [['R'], [ordinary]], 'RateParams': [['R'], [ordinary]], 'RateDerived': [['R'], [ordinary]],
                'RateFromPayment': [['R'], [.01]], 'RateRows': [['Period', 'R'], *[[v, .01] for v in values]],
                'RateGrouped': [['G', 'R'], [0, ordinary], [1, ordinary]],
                'RateCrosstab': [['G', 'Total', '1', '2', '4'], [0, .01, None, .01, .01], [1, .01, .01, None, None]],
                'RateUnion': [['R'], [ordinary], [-.05]],
            }
            refusals = {'RateNoConvergence', 'RateImpossible', 'RateLate'}
            names = java('SavedReadOracle', path).splitlines()
            assert len(names) == 25
            names = [name for name in names if name.startswith('Rate')]
            assert set(names) == set(expected) | refusals
            for name in names:
                raw = java('SavedQueryOracle', path, name)
                inspected = cli('inspect', path, '--query', name)
                assert list(csv.reader(StringIO(inspected), delimiter='\t', strict=True)) == [line.split('\t') for line in raw.splitlines()]
                sql = java('SavedReadOracle', path, name)
                assert cli('query', 'saved', '--database', path, name) == cli('query', 'parse', sql)
                controls += 2
                for query in [sql, None, 'SELECT * FROM [' + name + ']']:
                    command = ['query', 'run'] + (['--saved', name] if query is None else [query]) + ['--database', path]
                    if name == 'RateParams':
                        command += ['--param', 'GuessBp=long:500']
                    result = cli(*command, accepted=name not in refusals)
                    if name not in refusals:
                        verify(result, expected[name])
                    controls += 1
            assert path.read_bytes() == original
            print(path.name, len(original), hashlib.sha256(original).hexdigest(), flush=True)
    print(f'Passed {controls} rate/certificate/scalar/getter/rendering/direct/saved/named/refusal controls; native Access unrun.')


if __name__ == '__main__':
    main()
