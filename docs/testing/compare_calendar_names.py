"""Independent explicit-language month/weekday naming and week-numbering controls.

Jackcess supplies optional native row inputs, not native Access SQL evaluation.
"""
import argparse
import csv
import datetime as dt
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, run
from compare_cstr import spelling
from compare_date_format import EN_MONTHS, EN_WEEKDAYS, DE_MONTHS, DE_WEEKDAYS
from compare_large_tables import digest


def month_name(month, short, profile):
    name = (DE_MONTHS if profile == 'de-de' else EN_MONTHS)[month-1]
    return name[:3] if short else name


def weekday_name(day, first, short, profile):
    date = dt.date(2023, 1, 1)+dt.timedelta(days=day-1+first-1)
    name = (DE_WEEKDAYS if profile == 'de-de' else EN_WEEKDAYS)[date.weekday()]
    return name[:(2 if profile == 'de-de' else 3)] if short else name


def check(cli, mode, profile, cases, first=None):
    for start in range(0, len(cases), 6):
        batch = cases[start:start+6]
        sql = 'SELECT '+','.join(f'{expression} AS C{i}' for i,(expression,_) in enumerate(batch))
        options = [] if first is None else ['--first-day-of-week', str(first)]
        result = subprocess.run([str(cli), 'query', mode, sql, '--date-format', profile, *options], capture_output=True, text=True)
        assert result.returncode == 0, (mode, profile, sql, result.stderr)
        rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
        assert len(rows) == 2 and rows[1] == [spelling(value) for _,value in batch], (mode, profile, batch, rows)
    return len(cases)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    controls = 0
    for mode in ['eval', 'run']:
        for profile in ['iso', 'en-us', 'de-de']:
            cases = [(f'MonthName({month},{str(short)})', month_name(month, short, profile))
                     for month in range(1, 13) for short in [False, True]]
            for first in range(1, 8):
                for day in range(1, 8):
                    for short in [False, True]:
                        cases.append((f'WeekdayName({day},{str(short)},{first})', weekday_name(day, first, short, profile)))
            controls += check(args.cli, mode, profile, cases)
            for first in range(1, 8):
                cases = [(f'WeekdayName({day},{str(short)})', weekday_name(day, first, short, profile))
                         for day in range(1, 8) for short in [False, True]]
                cases.extend([('WeekdayName(1,,0)', weekday_name(1, first, False, profile)),
                              ('WeekdayName(1,,1)', weekday_name(1, 1, False, profile)),
                              ('WeekdayName(1,,)', weekday_name(1, first, False, profile))])
                controls += check(args.cli, mode, profile, cases, first)
        for expression in ['MonthName(NULL)', 'MonthName(0)', 'MonthName(13)', 'MonthName(1,NULL)',
                           'WeekdayName(1)', 'WeekdayName(1,False,0)', 'WeekdayName(8,False,1)',
                           'WeekdayName(1,False,8)', 'MonthName(1/0)', 'VBA.MonthName(1)']:
            result = subprocess.run([str(args.cli), 'query', mode, 'SELECT 1,'+expression, '--date-format', 'en-us'], capture_output=True)
            assert result.returncode != 0 and not result.stdout, expression
            controls += 1
    if args.jar:
        assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before = digest(args.database)
        tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar, RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
        identities = sorted(int(line.split('\t')[1].removeprefix('I32:')) for line in tagged.splitlines())
        sql = 'SELECT ORDR_NBR AS N,MonthName((ORDR_NBR MOD 12)+1,True) AS M,WeekdayName((ORDR_NBR MOD 7)+1,False,2) AS D FROM ROULETTE_TB ORDER BY N'
        for profile in ['iso', 'en-us', 'de-de']:
            expected = [[str(identity), spelling(month_name(identity % 12 + 1, True, profile)),
                         spelling(weekday_name(identity % 7 + 1, 2, False, profile))] for identity in identities]
            for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
                result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(args.database), '--date-format', profile, *options], capture_output=True, text=True)
                assert result.returncode == 0, result.stderr
                rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
                assert rows[1:] == expected, (profile, options, rows)
                controls += len(expected)*2
        assert digest(args.database) == before
    print(f'Passed {controls} independent calendar-name/refusal/native-row controls; native Access unrun.')


if __name__ == '__main__':
    main()
