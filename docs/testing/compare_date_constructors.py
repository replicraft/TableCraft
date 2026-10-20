"""Compare Gregorian date/time constructors with independent Python models.

No Microsoft Access execution is claimed. Native input supplies only inert Int32
rows through the reviewed independent reader; its source hash must stay unchanged.
"""
import argparse
import calendar
import datetime
from fractions import Fraction
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES, run
from compare_cached_pages import date_display
from compare_large_tables import digest

ANCHOR = datetime.date(2000, 1, 1)


def date_serial(year, month, day):
    if 0 <= year <= 99:
        year += 2000 if year <= 29 else 1900
    year, month = year + (month-1)//12, (month-1)%12+1
    # Count independent Python calendar years, not the production closed formula.
    years = sum(366 if calendar.isleap(y) else 365 for y in range(min(year, 2000), max(year, 2000)))
    if year < 2000:
        years = -years
    days = years + sum(calendar.monthrange(year, m)[1] for m in range(1, month)) + day-1
    value = ANCHOR + datetime.timedelta(days=days)
    assert 100 <= value.year <= 9999
    return '#' + value.isoformat() + ' 00:00:00.000000000#'


def time_serial(hour, minute, second):
    value = float(Fraction(hour*3600+minute*60+second, 86400))
    return date_display('Y64:' + struct.pack('>d', value).hex())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--database', type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    before = digest(args.database)
    dates = [(1969,2,12),(1980,6,0),(2024,3,0),(2023,3,0),(2024,13,1),(2024,0,1),(0,1,1),(29,1,1),(30,1,1),(99,1,1),(100,1,1),(9999,12,31),(10050,1,-32768),(2024,-24,400),(2024,24,-400)]
    times = [(16,35,17),(6,-15,0),(0,75,0),(25,0,0),(-1,0,0),(0,0,-1),(-25,0,0),(0,-75,0),(32767,32767,32767),(-32768,-32768,-32768)]
    controls = 0
    for mode in ['eval', 'run']:
        for name, cases, model in [('DateSerial', dates, date_serial), ('TimeSerial', times, time_serial)]:
            for values in cases:
                sql = 'SELECT '+name+'('+','.join(map(str,values))+') AS D'
                assert run(args.cli, 'query', mode, sql) == 'D\n'+model(*values)+'\n', (mode, name, values)
                controls += 1
        for sql, expected in [('SELECT DateSerial(2024,CDec(2.5),28.5) AS D', date_serial(2024,2,28)), ('SELECT TimeSerial(0,1.5,0) AS D', time_serial(0,2,0))]:
            assert run(args.cli, 'query', mode, sql) == 'D\n'+expected+'\n'
            controls += 1
        for expression in ['DateSerial(NULL,1,1)', 'TimeSerial(NULL,0,0)', 'DateSerial(100,1,0)', 'DateSerial(9999,12,32)', 'DateSerial(32767,32767,32767)', 'TimeSerial(32768,0,0)', 'DateSerial(2024,,1)', 'TimeSerial(1,2)', 'DateSerial(NULL,1,1/0)']:
            result = subprocess.run([str(args.cli),'query',mode,'SELECT 1 AS Before,'+expression+' AS D'], capture_output=True)
            assert result.returncode != 0 and result.stdout == b'', (mode, expression)
            controls += 1
    tagged = run('java','-Dstdout.encoding=UTF-8','-cp',args.jar,RECIPES/'TableDataOracle.java',args.database,'ROULETTE_TB')
    ids = sorted(int(line.split('\t')[1].removeprefix('I32:')) for line in tagged.splitlines())
    sql = 'SELECT ORDR_NBR AS N,DateSerial(2000+ORDR_NBR,2,29) AS D FROM ROULETTE_TB ORDER BY N'
    expected = 'N\tD\n'+''.join(str(n)+'\t'+date_serial(2000+n,2,29)+'\n' for n in ids)
    for options in [[],['--index','ROULETTE_TB','Number']]:
        assert run(args.cli,'query','run',sql,'--database',args.database,*options) == expected
        controls += 1
    assert digest(args.database) == before
    print(f'Passed {controls} Gregorian constructor model controls; unchanged source {before}; native Access unrun.')


if __name__ == '__main__':
    main()
