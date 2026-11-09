"""Check explicit FormatDateTime profiles with Python Gregorian models.

Optional Jackcess input checks do not execute or qualify native Access expressions.
"""
import argparse
import csv
import datetime as dt
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run
from compare_cstr import date_text, spelling
from compare_large_tables import digest

EN_MONTHS = ('January February March April May June July August September October November December').split()
EN_WEEKDAYS = ('Monday Tuesday Wednesday Thursday Friday Saturday Sunday').split()
DE_MONTHS = ('Januar Februar März April Mai Juni Juli August September Oktober November Dezember').split()
DE_WEEKDAYS = ('Montag Dienstag Mittwoch Donnerstag Freitag Samstag Sonntag').split()


def expected(value, profile, style):
    if style == 0:
        return date_text(value, profile)
    if style == 4:
        return f'{value.hour:02}:{value.minute:02}'
    if style == 3:
        return date_text(value.replace(year=1899, month=12, day=30), profile)
    if style == 1 and profile == 'en-us':
        return f'{EN_WEEKDAYS[value.weekday()]}, {EN_MONTHS[value.month-1]} {value.day}, {value.year:04}'
    if style == 1 and profile == 'de-de':
        return f'{DE_WEEKDAYS[value.weekday()]}, {value.day}. {DE_MONTHS[value.month-1]} {value.year:04}'
    if profile == 'en-us':
        return f'{value.month}/{value.day}/{value.year:04}'
    if profile == 'de-de':
        return f'{value.day:02}.{value.month:02}.{value.year:04}'
    return f'{value.year:04}-{value.month:02}-{value.day:02}'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    rng = random.Random(20261020)
    values = [dt.datetime(1899, 12, 30), dt.datetime(2024, 2, 29), dt.datetime(100, 1, 1),
              dt.datetime(9999, 12, 31, 23, 59, 59)]
    for _ in range(96):
        values.append(dt.datetime(1900, 1, 1)+dt.timedelta(days=rng.randrange(73000), seconds=rng.randrange(86400)))
    controls = 0
    for mode in ['eval', 'run']:
        for profile in ['iso', 'en-us', 'de-de']:
            for value in values:
                literal = f'#{value.year:04}-{value.month:02}-{value.day:02} {value.hour:02}:{value.minute:02}:{value.second:02}#'
                sql = 'SELECT '+','.join(f'FormatDateTime({literal},{style}) AS F{style}' for style in range(5))
                result = subprocess.run([str(args.cli), 'query', mode, sql, '--date-format', profile], capture_output=True, text=True)
                assert result.returncode == 0, (mode, profile, value, result.stderr)
                rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
                assert len(rows) == 2
                assert rows[1] == [spelling(expected(value, profile, style)) for style in range(5)], (mode, profile, value, rows)
                controls += 5
        for expression in ['FormatDateTime(NULL)', 'FormatDateTime(1,5)', 'FormatDateTime(1,1/0)',
                           'FormatDateTime(1/0)', 'VBA.FormatDateTime(1)']:
            result = subprocess.run([str(args.cli), 'query', mode, 'SELECT 1,'+expression, '--date-format', 'iso'], capture_output=True)
            assert result.returncode != 0 and not result.stdout, expression
            controls += 1
    if args.jar:
        assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before = digest(args.database)
        tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar, RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
        expected_rows = []
        for line in tagged.splitlines():
            fields = line.split('\t')[1:]
            identity = int(fields[0].removeprefix('I32:'))
            value = dt.datetime(1899, 12, 30)+dt.timedelta(days=identity)
            expected_rows.append((identity, value))
        expected_rows.sort()
        for profile in ['iso', 'en-us', 'de-de']:
            for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
                sql = 'SELECT ORDR_NBR AS N,FormatDateTime(ORDR_NBR,1) AS D FROM ROULETTE_TB ORDER BY N'
                result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(args.database), '--date-format', profile, *options], capture_output=True, text=True)
                assert result.returncode == 0, result.stderr
                rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
                assert rows[1:] == [[str(identity), spelling(expected(value, profile, 1))] for identity, value in expected_rows]
                controls += len(expected_rows)
        assert digest(args.database) == before
    print(f'Passed {controls} independent date-format/refusal/native-row controls; native Access unrun.')


if __name__ == '__main__':
    main()
