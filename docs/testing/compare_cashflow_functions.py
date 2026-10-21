"""Independent Decimal ledgers and inert native cashflow-query metadata."""
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


def npv(rate, values):
    total = Decimal(0)
    discount = Decimal(1)
    for value in values:
        discount *= 1 + rate
        total += value / discount
    return total


def irr(values):
    # Monotone single-payment certificates only; independent bisection.
    low, high = Decimal('-.5'), Decimal('.5')
    for _ in range(300):
        middle = (low + high) / 2
        residual = values[0] + npv(middle, values[1:])
        if residual > 0:
            low = middle
        else:
            high = middle
    return (low + high) / 2


def mirr(finance, reinvest, values):
    end = len(values) - 1
    positive = sum(value * (1 + reinvest) ** (end - index)
                   for index, value in enumerate(values) if value > 0)
    negative = sum(-value / (1 + finance) ** index
                   for index, value in enumerate(values) if value < 0)
    return ((positive / negative).ln() / end).exp() - 1


def verify(output, expected):
    rows = list(csv.reader(StringIO(output), delimiter='\t', strict=True))
    assert rows[0] == expected[0] and len(rows) == len(expected), (rows, expected)
    for actual, wanted in zip(rows[1:], expected[1:]):
        assert len(actual) == len(wanted)
        for cell, value in zip(actual, wanted):
            if value is None:
                assert cell == 'NULL'
            else:
                actual_value = float(cell)
                expected_value = float(value)
                tolerance = max(1, abs(expected_value)) * 2e-10
                assert math.isfinite(actual_value) and abs(actual_value - expected_value) <= tolerance, (cell, value)
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
        for length in [2, 3, 12, 24]:
            for rate_text in ['-.05', '0', '.01', '.1']:
                rate = Decimal(rate_text)
                values = [Decimal(-1000)] + [Decimal(0)] * (length - 2)
                values.append(Decimal(1000) * (1 + rate) ** (length - 1))
                constructor = 'Array(' + ','.join(map(str, values)) + ')'
                expected = [['N', 'I', 'M'], [npv(rate, values), irr(values), mirr(Decimal('.05'), Decimal('.1'), values)]]
                sql = f'SELECT NPV({rate},{constructor}) AS N,IRR({constructor}) AS I,MIRR({constructor},.05,.1) AS M'
                for command in ['eval', 'run']:
                    rows = verify(cli('query', command, sql), expected)
                    actual_rate = Decimal(rows[1][1])
                    residual = values[0] + npv(actual_rate, values[1:])
                    assert abs(residual) <= Decimal('1e-8') * abs(values[0])
                    controls += 1

    for command in ['eval', 'run']:
        for sql in [
            'SELECT NPV(0,Array(1,1/0))',
            'SELECT 1,IRR(Array(-1,1,-1))',
            'SELECT 1,MIRR(Array(-1,2),-1,.1)',
            'SELECT 1,NPV(0,Array())',
        ]:
            cli('query', command, sql, accepted=False)
            controls += 1

    with tempfile.TemporaryDirectory(prefix='tablecraft-cashflow-oracle-') as classes:
        names = ['SavedForestFixture', 'SavedStatisticsFixture', 'SavedCashFlowFixture',
                 'SavedReadOracle', 'SavedQueryOracle', 'TableDataOracle']
        subprocess.run(['javac', '-encoding', 'UTF-8', '-cp', str(args.jar), '-d', classes,
                        *[str(RECIPES / (name + '.java')) for name in names]], check=True)

        def java(name, *values):
            return subprocess.check_output([
                'java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar) + os.pathsep + classes,
                name, *map(str, values),
            ], text=True, encoding='utf-8')

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java('SavedCashFlowFixture', args.directory / ('cashflows-' + suffix), profile)

        for _, suffix in PROFILES:
            path = args.directory / ('cashflows-' + suffix)
            original = path.read_bytes()
            values = [int(row.split('\t')[1].removeprefix('I32:'))
                      for row in java('TableDataOracle', path, 'AData').splitlines()]
            assert values == [1, 2, 4]
            expected = {
                'CashNpv': [['V'], [0]],
                'CashIrr': [['V'], [.1]],
                'CashMirr': [['V'], [.1]],
                'CashParams': [['V'], [21]],
                'CashRows': [['ID', 'V'], *[[v, v + 1] for v in values]],
                'CashGrouped': [['G', 'V'], [0, 8], [1, 2]],
                'CashCrosstab': [['G', 'Total', '1', '2', '4'], [0, 8, None, 3, 5], [1, 2, 2, None, None]],
                'CashDerived': [['V'], [5]],
                'CashUnion': [['V'], [5], [.21]],
            }
            refusals = {'CashEmpty', 'CashBadGuess', 'CashLate'}
            names = java('SavedReadOracle', path).splitlines()
            assert len(names) == 26
            names = [name for name in names if name.startswith('Cash')]
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
                    if name == 'CashParams':
                        command += ['--param', 'P=long:20']
                    result = cli(*command, accepted=name not in refusals)
                    if name not in refusals:
                        verify(result, expected[name])
                    controls += 1

            assert path.read_bytes() == original
            print(path.name, len(original), hashlib.sha256(original).hexdigest(), flush=True)

    print(f'Passed {controls} cashflow/certificate/scalar/getter/rendering/direct/saved/named/refusal controls; native Access unrun.')


if __name__ == '__main__':
    main()
