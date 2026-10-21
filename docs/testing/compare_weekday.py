"""Check standalone Weekday against Python Gregorian dates and serial day models.

Optional Jackcess row inputs do not execute native Access expressions.
"""
import argparse
import csv
import datetime as dt
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run
from compare_large_tables import digest
from compare_week_context import weekday


def check(cli, mode, first, cases):
    for start in range(0, len(cases), 6):
        batch = cases[start:start+6]
        sql = 'SELECT '+','.join(f'{expression} AS C{i}' for i,(expression,_) in enumerate(batch))
        result = subprocess.run([str(cli), 'query', mode, sql, '--first-day-of-week', str(first), '--date-format', 'iso', '--number-format', 'invariant', '--reference-year', '2024'], capture_output=True, text=True)
        assert result.returncode == 0, (mode, first, sql, result.stderr)
        rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
        assert len(rows) == 2 and rows[1] == [str(expected) for _,expected in batch], (mode, first, batch, rows)
    return len(cases)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    rng = random.Random(20261020)
    dates = [dt.date(1969, 2, 12), dt.date(100, 1, 1), dt.date(9999, 12, 31)]
    dates += [dt.date(1900, 1, 1)+dt.timedelta(days=rng.randrange(73000)) for _ in range(32)]
    count = 0
    for mode in ['eval', 'run']:
        for first in range(1, 8):
            cases = [('Weekday(NULL)', 'NULL'), ('Weekday(0)', 7), ("Weekday('Feb 29',0)", weekday(dt.date(2024, 2, 29), first))]
            for value in dates:
                cases.extend([(f'Weekday(#{value}#,0)', weekday(value, first)),
                              (f'Weekday(#{value}#)', weekday(value, 1)),
                              (f'Weekday(#{value}#,)', weekday(value, 1)),
                              (f"Weekday('{value}',{first})", weekday(value, first))])
            for day in [-657434, -1000, -1, 0, 1, 1000, 2958465]:
                value = dt.date(1899, 12, 30)+dt.timedelta(days=day)
                cases.extend([(f'Weekday({day},0)', weekday(value, first)),
                              (f"Weekday('{day}',0)", weekday(value, first))])
            count += check(args.cli, mode, first, cases)
        for expression in ['Weekday(1,0)', "Weekday('2024-02-29')", 'Weekday(1,NULL)', 'Weekday(1,8)', 'Weekday(NULL,1/0)', 'VBA.Weekday(1)']:
            result = subprocess.run([str(args.cli), 'query', mode, 'SELECT 1,'+expression], capture_output=True)
            assert result.returncode != 0 and not result.stdout, expression
            count += 1
    if args.jar:
        assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before = digest(args.database)
        tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar, RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
        identities = sorted(int(line.split('\t')[1].removeprefix('I32:')) for line in tagged.splitlines())
        sql = "SELECT ORDR_NBR AS N,Weekday(ORDR_NBR,0) AS W,Weekday(ORDR_NBR) AS D FROM ROULETTE_TB ORDER BY N"
        for first in [1, 2, 7]:
            expected = [[str(identity), str(weekday(dt.date(1899, 12, 30)+dt.timedelta(days=identity), first)),
                         str(weekday(dt.date(1899, 12, 30)+dt.timedelta(days=identity), 1))] for identity in identities]
            for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
                result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(args.database), '--first-day-of-week', str(first), *options], capture_output=True, text=True)
                assert result.returncode == 0, result.stderr
                rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
                assert rows[1:] == expected, (first, options, rows)
                count += len(expected)*2
        assert digest(args.database) == before
    print(f'Passed {count} independent weekday/refusal/native-row controls; native Access unrun.')


if __name__ == '__main__':
    main()
