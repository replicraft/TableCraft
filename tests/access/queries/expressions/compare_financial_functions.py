"""Independent Decimal cashflow ledgers plus original inert native query definitions.

Jackcess reads/writes metadata and renders SQL, never evaluates financial calls.
"""
import argparse
import csv
from decimal import Decimal, localcontext
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES
from compare_saved_forests import PROFILES
from compare_saved_statistics import verify


def ledger(rate, periods, begin):
    # Build growth and unit-payment accumulation one period at a time. This does
    # not reuse the implementation's logarithmic/expm1 closed forms.
    with localcontext() as context:
        context.prec = 400
        factor = 1 + Decimal(rate)
        growth, payments = Decimal(1), Decimal(0)
        for _ in range(periods):
            if begin:
                payments += 1
            growth *= factor
            payments *= factor
            if not begin:
                payments += 1
        return [-Decimal(1000) * growth / payments,
                Decimal(100) * payments / growth,
                Decimal(100) * payments]


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
            assert not result.stdout, (values, result.stdout)
        return result.stdout

    headings = ['Payment', 'Present', 'Future']
    for rate in ['-0.05', '0', '1e-20', '.01', '.1']:
        for periods in [1, 12, 60]:
            for begin in [0, 1]:
                sql = f"SELECT Pmt({rate},{periods},1000,,{begin}) AS Payment,PV({rate},{periods},-100,,{begin}) AS Present,FV({rate},{periods},-100,,{begin}) AS Future"
                for command in ['eval', 'run']:
                    verify(cli('query', command, sql), [headings, ledger(rate, periods, begin)])
                    controls += 1
    for command in ['eval', 'run']:
        for sql in ["SELECT 1,Pmt(Null,12,1000,1/0)", "SELECT 1,Pmt(0,0,1000)", "SELECT 1,FV(0,10,-100,0,2)"]:
            cli('query', command, sql, accepted=False)
            controls += 1

    with tempfile.TemporaryDirectory(prefix='tablecraft-financial-oracle-') as classes:
        names = ['SavedForestFixture', 'SavedStatisticsFixture', 'SavedFinancialFixture', 'SavedReadOracle', 'SavedQueryOracle', 'TableDataOracle']
        subprocess.run(['javac', '-encoding', 'UTF-8', '-cp', str(args.jar), '-d', classes,
                        *[str(RECIPES / (name + '.java')) for name in names]], check=True)

        def java(name, *values):
            return subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar) + os.pathsep + classes,
                                           name, *map(str, values)], text=True, encoding='utf-8')

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java('SavedFinancialFixture', args.directory / ('financial-' + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ('financial-' + suffix)
            original = path.read_bytes()
            values = [int(row.split('\t')[1].removeprefix('I32:')) for row in java('TableDataOracle', path, 'AData').splitlines()]
            assert values == [1, 2, 4]
            expected = {
                'FinanceAll': [headings, ledger('.01', 12, 0)],
                'FinanceParams': [headings, ledger('.01', 12, 1)],
                'FinanceGrouped': [['G', 'V'], [0, 60], [1, 10]],
                'FinanceDerived': [['V'], *[[-120 / value] for value in values]],
                'FinanceUnion': [['V'], [-10], [1000]],
                'FinanceCrosstab': [['G', 'Total', '1', '2', '4'], [0, 60, None, 20, 40], [1, 10, 10, None, None]],
            }
            refusals = {'FinanceLateError', 'FinanceWrongType', 'FinanceNull'}
            names = java('SavedReadOracle', path).splitlines()
            assert len(names) == 32
            names = [name for name in names if name.startswith('Finance')]
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
                    if name == 'FinanceParams':
                        command += ['--param', 'BasisPoints=long:100', '--param', 'Periods=long:12']
                    result = cli(*command, accepted=name not in refusals)
                    if name not in refusals:
                        verify(result, expected[name])
                    controls += 1
            assert path.read_bytes() == original
            print(path.name, len(original), hashlib.sha256(original).hexdigest(), flush=True)
    print(f'Passed {controls} financial ledger/scalar/getter/rendering/direct/saved/named/refusal controls; native Access unrun.')


if __name__ == '__main__':
    main()
