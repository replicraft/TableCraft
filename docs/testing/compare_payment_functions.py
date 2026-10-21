"""Independent Decimal amortization ledgers and original inert native query metadata."""
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


def amortize(rate, periods, present, future, begin):
    with localcontext() as context:
        context.prec = 400
        rate, present, future = map(Decimal, (rate, present, future))
        growth, accumulation = Decimal(1), Decimal(0)
        for _ in range(periods):
            growth *= 1 + rate
            if begin:
                accumulation += 1
            accumulation *= 1 + rate
            if not begin:
                accumulation += 1
        payment = -(present * growth + future) / accumulation
        balance, prior_interest = present, Decimal(0)
        rows = []
        for per in range(1, periods + 1):
            interest = (-prior_interest if per > 1 else Decimal(0)) if begin else -balance * rate
            rows.append([per, interest, payment - interest, payment, periods])
            if begin:
                balance += payment
                prior_interest = balance * rate
                balance += prior_interest
            else:
                balance = balance * (1 + rate) + payment
        assert abs(balance + future) < Decimal('1e-350')
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
            assert not result.stdout, (values, result.stdout)
        return result.stdout

    for rate in ['-.05', '0', '1e-20', '.01', '.1']:
        for periods in [1, 12, 60]:
            for begin in [0, 1]:
                rows = amortize(rate, periods, '1000', '0', begin)
                for per in sorted({1, periods}):
                    sql = f"SELECT IPmt({rate},{per},{periods},1000,,{begin}) AS Interest,PPmt({rate},{per},{periods},1000,,{begin}) AS Principal,NPer({rate},Pmt({rate},{periods},1000,,{begin}),1000,,{begin}) AS Periods"
                    expected = [['Interest', 'Principal', 'Periods'], [*rows[per-1][1:3], periods]]
                    for command in ['eval', 'run']:
                        try:
                            verify(cli('query', command, sql), expected)
                        except AssertionError as error:
                            raise AssertionError((sql, error.args)) from error
                        controls += 1
    for command in ['eval', 'run']:
        for sql in ["SELECT 1,IPmt(.01,0,12,1000)", "SELECT 1,PPmt(Null,1,12,1000,1/0)", "SELECT 1,NPer(.01,-1,1000)"]:
            cli('query', command, sql, accepted=False)
            controls += 1
    with tempfile.TemporaryDirectory(prefix='tablecraft-payment-oracle-') as classes:
        names = ['SavedForestFixture', 'SavedStatisticsFixture', 'SavedFinancialFixture', 'SavedPaymentFixture', 'SavedReadOracle', 'SavedQueryOracle', 'TableDataOracle']
        subprocess.run(['javac', '-encoding', 'UTF-8', '-cp', str(args.jar), '-d', classes,
                        *[str(RECIPES / (name + '.java')) for name in names]], check=True)

        def java(name, *values):
            return subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar) + os.pathsep + classes,
                                           name, *map(str, values)], text=True, encoding='utf-8')

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java('SavedPaymentFixture', args.directory / ('payments-' + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ('payments-' + suffix)
            original = path.read_bytes()
            values = [int(row.split('\t')[1].removeprefix('I32:')) for row in java('TableDataOracle', path, 'AData').splitlines()]
            assert values == [1, 2, 4]
            end, begin = amortize('.01', 4, '1000', '0', False), amortize('.01', 4, '1000', '0', True)
            with localcontext() as context:
                context.prec = 400
                expected = {
                    'PeriodEnd': [['Period', 'Interest', 'Principal', 'Payment', 'Periods'], *[end[v-1] for v in values]],
                    'PeriodBegin': [['Period', 'Interest', 'Principal', 'Payment', 'Periods'], *[begin[v-1] for v in values]],
                    'PeriodParams': [['Interest', 'Principal'], begin[1][1:3]],
                    'PeriodGrouped': [['G', 'Interest', 'Principal'], *[[g, *[sum(end[v-1][column] for v in values if v % 2 == g) for column in [1, 2]]] for g in [0, 1]]],
                    'PeriodCrosstab': [['G', 'Total', '1', '2', '4'], [0, end[1][1]+end[3][1], None, end[1][1], end[3][1]], [1, end[0][1], end[0][1], None, None]],
                    'PeriodDerived': [['Interest'], *[[end[v-1][1]] for v in values]],
                    'PeriodUnion': [['V'], [end[0][1]], [end[3][1]]],
                }
            refusals = {'PeriodInvalid', 'PeriodImpossible', 'PeriodNull'}
            names = java('SavedReadOracle', path).splitlines()
            assert len(names) == 42
            names = [name for name in names if name.startswith('Period')]
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
                    if name == 'PeriodParams':
                        command += ['--param', 'Per=long:2']
                    result = cli(*command, accepted=name not in refusals)
                    if name not in refusals:
                        verify(result, expected[name])
                    controls += 1
            assert path.read_bytes() == original
            print(path.name, len(original), hashlib.sha256(original).hexdigest(), flush=True)
    print(f'Passed {controls} period/payment ledger/scalar/getter/rendering/direct/saved/named/refusal controls; native Access unrun.')


if __name__ == '__main__':
    main()
