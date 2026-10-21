"""Check explicit system week policies against independent Gregorian models.

The documented final-Monday DatePart anomaly is retained. Windows Access is unrun.
"""
import argparse
import csv
import datetime as dt
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, run
from compare_large_tables import digest


def weekday(value, first):
    return (value.weekday() - ((first+5) % 7)) % 7 + 1


def first_start(year, first, rule):
    # Locate the first qualifying start by calendar membership, not ordinal offsets.
    value = dt.date(year, 1, 1)-dt.timedelta(days=6)
    for _ in range(14):
        if weekday(value, first) == 1:
            members = [value+dt.timedelta(days=day) for day in range(7)]
            belongs = sum(member.year == year for member in members)
            if ((rule == 1 and dt.date(year, 1, 1) in members)
                    or (rule == 2 and belongs >= 4)
                    or (rule == 3 and belongs == 7)):
                return value
        value += dt.timedelta(days=1)
    raise AssertionError((year, first, rule))


def week_number(value, first, rule):
    start = first_start(value.year, first, rule)
    if value < start:
        start = first_start(value.year-1, first, rule)
    elif rule == 2 and value >= first_start(value.year+1, first, rule):
        if not (first == 2 and value.weekday() == 0):
            return 1
    # Count enumerated weekly starts through the supplied date.
    count = 0
    while start <= value:
        count += 1
        start += dt.timedelta(days=7)
    return count


def boundaries(first_date, last_date, first_day):
    if first_date > last_date:
        return -boundaries(last_date, first_date, first_day)
    value = first_date+dt.timedelta(days=1)
    count = 0
    while value <= last_date:
        count += weekday(value, first_day) == 1
        value += dt.timedelta(days=1)
    return count


def execute(cli, mode, first, rule, cases):
    for start in range(0, len(cases), 6):
        batch = cases[start:start+6]
        sql = 'SELECT '+','.join(f'{expression} AS C{i}' for i,(expression,_) in enumerate(batch))
        result = subprocess.run([str(cli), 'query', mode, sql, '--first-day-of-week', str(first), '--first-week-of-year', str(rule)], capture_output=True, text=True)
        assert result.returncode == 0, (mode, first, rule, sql, result.stderr)
        rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
        assert len(rows) == 2 and rows[1] == [str(expected) for _,expected in batch], (mode, first, rule, batch, rows)
    return len(cases)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    dates = [dt.date(2003, 12, 29), dt.date(2003, 12, 30), dt.date(2021, 1, 1),
             dt.date(2024, 12, 31), dt.date(2026, 1, 1), dt.date(2026, 10, 20)]
    first_date, last_date = dt.date(2026, 10, 18), dt.date(2026, 10, 26)
    count = 0
    for mode in ['eval', 'run']:
        for first in range(1, 8):
            for rule in range(1, 4):
                cases = []
                for value in dates:
                    cases.extend([(f"DatePart('w',#{value}#,0)", weekday(value, first)),
                                  (f"DatePart('ww',#{value}#,0,0)", week_number(value, first, rule)),
                                  (f"DatePart('ww',#{value}#)", week_number(value, 1, 1)),
                                  (f"DatePart('ww',#{value}#,1,1)", week_number(value, 1, 1))])
                cases.extend([(f"DateDiff('ww',#{first_date}#,#{last_date}#,0,0)", boundaries(first_date, last_date, first)),
                              (f"DateDiff('ww',#{last_date}#,#{first_date}#,0,0)", -boundaries(first_date, last_date, first)),
                              (f"DateDiff('ww',#{first_date}#,#{last_date}#)", boundaries(first_date, last_date, 1))])
                count += execute(args.cli, mode, first, rule, cases)
        for expression in ["DatePart('ww',#2021-01-01#,0,0)", "DateDiff('ww',#2026-10-18#,#2026-10-19#,0,0)", "DatePart('w',#2026-10-20#,NULL)"]:
            result = subprocess.run([str(args.cli), 'query', mode, 'SELECT 1,'+expression, '--first-day-of-week', '2'], capture_output=True)
            assert result.returncode != 0 and not result.stdout, expression
            count += 1
    if args.jar:
        assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before = digest(args.database)
        tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar, RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
        identities = sorted(int(line.split('\t')[1].removeprefix('I32:')) for line in tagged.splitlines())
        for first, rule in [(1, 1), (2, 2), (7, 3)]:
            expected = [[str(identity), str(week_number(dt.date(2020, 12, 28)+dt.timedelta(days=identity), first, rule))] for identity in identities]
            sql = "SELECT ORDR_NBR AS N,DatePart('ww',DateAdd('d',ORDR_NBR,#2020-12-28#),0,0) AS W FROM ROULETTE_TB ORDER BY N"
            for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
                result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(args.database), '--first-day-of-week', str(first), '--first-week-of-year', str(rule), *options], capture_output=True, text=True)
                assert result.returncode == 0, result.stderr
                rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
                assert rows[1:] == expected, (first, rule, options, rows)
                count += len(expected)
        assert digest(args.database) == before
    print(f'Passed {count} independent week-policy/refusal/native-row controls; native Access unrun.')


if __name__ == '__main__':
    main()
