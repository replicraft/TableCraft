"""Independent IsNumeric/IsDate controls over generated strings and typed values.

This checks the documented explicit profiles, not native Access expression semantics.
"""
import argparse
import csv
from datetime import date
from decimal import Decimal
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run
from compare_large_indexes import INPUT_HASHES
from compare_large_tables import digest
from compare_error_messages import execute
from compare_number_format import regional
from compare_string_search import quoted


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    parser.add_argument('--native-directory', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    rng = random.Random(20261020)
    count = 0
    for mode in ['eval', 'run']:
        for profile in ['iso', 'en-us', 'de-de']:
            cases = [('IsDate(NULL)', False), ('IsDate(False)', False),
                     ('IsDate(0)', True), ('IsDate(#2026-10-20#)', True),
                     ("IsDate('Feb 29')", True), ("IsDate('4:28 PM')", True),
                     ("IsDate('123')", False), ("IsDate('2024-02-29 25:00')", False)]
            if profile == 'en-us':
                cases.extend([("IsDate('04.28.14')", True), ("IsDate('04.28.2014')", False)])
                for _ in range(64):
                    hour, minute, second = rng.randrange(24), rng.randrange(60), rng.randrange(60)
                    spelling = f'{hour:02}.{minute:02}.{second:02}'
                    cases.extend([(f'IsDate({quoted(spelling)})', True),
                                  (f'IsDate({quoted(spelling+".5")})', False)])
            for _ in range(128):
                year, month, day = rng.randrange(100, 10000), rng.randrange(1, 13), rng.randrange(1, 32)
                try:
                    date(year, month, day)
                    expected = True
                except ValueError:
                    expected = False
                spelling = {'iso': f'{year:04}-{month:02}-{day:02}',
                            'en-us': f'{month}/{day}/{year:04}',
                            'de-de': f'{day}.{month}.{year:04}'}[profile]
                cases.extend([(f'IsDate({quoted(spelling)})', expected),
                              (f'IsDate({quoted(spelling+" junk")})', False)])
            for start in range(0, len(cases), 8):
                batch = cases[start:start+8]
                sql = 'SELECT '+','.join(f'{expression} AS C{i}' for i,(expression,_) in enumerate(batch))
                result = subprocess.run([str(args.cli), 'query', mode, sql, '--date-format', profile,
                                         '--reference-year', '2024'], capture_output=True, text=True)
                assert result.returncode == 0, (mode, profile, sql, result.stderr)
                rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
                assert len(rows) == 2 and len(rows[1]) == len(batch)
                assert rows[1] == ['TRUE' if expected else 'FALSE' for _,expected in batch], (profile, batch, rows)
                count += len(batch)
        for profile in ['invariant', 'en-us', 'de-de']:
            cases = [('IsNumeric(0)', True), ('IsNumeric(False)', True), ('IsNumeric(NULL)', False),
                     ('IsNumeric(#2026-10-20#)', False),
                     ('IsNumeric({guid {00000000-0000-0000-0000-000000000000}})', False)]
            for _ in range(256):
                text = format(Decimal(rng.randrange(-(2**63), 2**63)).scaleb(-rng.randrange(12)), 'f')
                assert Decimal(text).is_finite()
                spelling = regional(text, profile)
                cases.extend([(f'IsNumeric({quoted(spelling)})', True),
                              (f'IsNumeric({quoted(spelling+" Help")})', False)])
            for text, expected in [('53', True), ('459.95' if profile != 'de-de' else '459,95', True),
                                   ('1e9999', False), ('1e-9999', True), ('45 Help', False),
                                   ('True', False), ('#TRUE#', False), ('NaN', False), ('', False),
                                   ('&HFF', False), ('1%', False), ('1 2', False)]:
                cases.append((f'IsNumeric({quoted(text)})', expected))
            for start in range(0, len(cases), 8):
                batch = cases[start:start+8]
                sql = 'SELECT '+','.join(f'{expression} AS C{i}' for i,(expression,_) in enumerate(batch))
                result = subprocess.run([str(args.cli), 'query', mode, sql, '--number-format', profile], capture_output=True, text=True)
                assert result.returncode == 0, (mode, profile, sql, result.stderr)
                rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
                assert len(rows) == 2 and len(rows[1]) == len(batch)
                assert rows[1] == ['TRUE' if expected else 'FALSE' for _,expected in batch], (profile, batch, rows)
                count += len(batch)
        for expression in ["IsNumeric('53')", 'IsNumeric(1/0)', 'IsNumeric(1,1/0)', 'VBA.IsNumeric(1)',
                           "IsDate('2024-02-29')", 'IsDate(1/0)', 'IsDate(1,1/0)', 'VBA.IsDate(1)']:
            result = subprocess.run([str(args.cli), 'query', mode, 'SELECT 1,'+expression], capture_output=True)
            assert result.returncode != 0 and not result.stdout, expression
            count += 1
    for mode in ['eval', 'run']:
        for code in [0, 1, 5, 2000, 2001, 32767, 32768, 65535]:
            sql = f'SELECT IsNumeric(CVErr({code})),IsDate(CVErr({code})),IsNumeric(CLng(CVErr({code}))),IsDate(CDate(CLng(CVErr({code}))))'
            assert execute(args.cli, mode, sql) == [['FALSE', 'FALSE', 'TRUE', 'TRUE']]
            count += 4
        for function in ['IsNumeric', 'IsDate']:
            for expression in ['1/0', 'IIf(True,CVErr(1),1/0)']:
                result = subprocess.run([str(args.cli), 'query', mode, f'SELECT 1,{function}({expression})'], capture_output=True, text=True)
                assert result.returncode == 2 and not result.stdout and 'division by zero' in result.stderr, result
                count += 1
    if args.native_directory:
        controls = ['--index', 'IndexedRows', 'IdAsc', '--native-bytes', '67108864',
                    '--native-rows', '100000', '--native-steps', '10000000', '--execution-steps', '10000000']
        for suffix, expected_hash in INPUT_HASHES.items():
            database = args.native_directory / ('index-' + suffix)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == expected_hash
            sql = 'SELECT IsNumeric(CVErr(ID)),IsDate(CVErr(ID)),IsNumeric(CLng(CVErr(ID))),IsDate(Stamp) FROM IndexedRows WHERE ID=1'
            assert execute(args.cli, 'run', sql, database, controls=controls) == [['FALSE', 'FALSE', 'TRUE', 'TRUE']]
            count += 4
            sql = 'SELECT IsNumeric(First(CVErr(ID))),IsDate(Max(CVErr(ID))),Count(CVErr(ID)) FROM IndexedRows WHERE ID=1'
            assert execute(args.cli, 'run', sql, database, controls=controls) == [['FALSE', 'FALSE', '1']]
            count += 3
            sql = 'SELECT 1,IsNumeric(IIf(True,CVErr(ID),1/(ID-1))) FROM IndexedRows WHERE ID=1'
            result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(database), *controls], capture_output=True, text=True)
            assert result.returncode == 2 and not result.stdout and 'division by zero' in result.stderr, result
            count += 1
            assert hashlib.sha256(database.read_bytes()).hexdigest() == expected_hash
    if args.jar:
        assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before = digest(args.database)
        tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar, RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
        expected = []
        for line in tagged.splitlines():
            row = line.split('\t')[1:]
            identity = int(row[0].removeprefix('I32:'))
            text = bytes.fromhex(row[1].removeprefix('T')).decode('utf-8')
            assert Decimal(text).is_finite()
            expected.append([str(identity), 'TRUE', 'FALSE', 'TRUE', 'FALSE'])
        expected.sort(key=lambda row: int(row[0]))
        sql = "SELECT ORDR_NBR AS N,IsNumeric(NBR_VALUE) AS Good,IsNumeric(NBR_VALUE & ' Help') AS Bad,IsDate(CDate(ORDR_NBR)) AS D,IsDate(NBR_VALUE & ' Help') AS X FROM ROULETTE_TB ORDER BY N"
        for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
            for profile in ['invariant', 'en-us', 'de-de']:
                result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(args.database), '--number-format', profile, '--date-format', 'iso', *options], capture_output=True, check=True, text=True)
                assert list(csv.reader(StringIO(result.stdout), delimiter='\t'))[1:] == expected
                count += len(expected)*4
        assert digest(args.database) == before
    print(f'Passed {count} independent inspection/refusal/native-row controls; native Access unrun.')


if __name__ == '__main__':
    main()
