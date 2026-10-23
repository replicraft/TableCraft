"""Compare Gregorian date/time constructors with independent Python models.

No Microsoft Access execution is claimed. Optional reviewed getter inputs and
registered authored fixtures supply inert Int32 fields; source hashes stay unchanged.
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
from compare_large_indexes import INPUT_HASHES

ANCHOR = datetime.date(2000, 1, 1)


def date_serial(year, month, day, last=2029):
    if 0 <= year <= 99:
        year += (last // 100) * 100
        if year > last:
            year -= 100
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
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    parser.add_argument('--native-directory', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    if args.jar:
        assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'

    dates = [(1969,2,12),(1980,6,0),(2024,3,0),(2023,3,0),(2024,13,1),(2024,0,1),(0,1,1),(29,1,1),(30,1,1),(99,1,1),(100,1,1),(9999,12,31),(10050,1,-32768),(2024,-24,400),(2024,24,-400)]
    times = [(16,35,17),(6,-15,0),(0,75,0),(25,0,0),(-1,0,0),(0,0,-1),(-25,0,0),(0,-75,0),(32767,32767,32767),(-32768,-32768,-32768)]
    controls = 0
    for mode in ['eval', 'run']:
        for name, cases, model in [('DateSerial', dates, date_serial), ('TimeSerial', times, time_serial)]:
            for values in cases:
                sql = 'SELECT '+name+'('+','.join(map(str,values))+') AS D'
                assert run(args.cli, 'query', mode, sql) == 'D\n'+model(*values)+'\n', (mode, name, values)
                controls += 1
        for last in [199,2029,2049,9999]:
            for year in [0,29,30,49,50,99]:
                sql=f'SELECT DateSerial({year},2,29) AS D'
                assert run(args.cli,'query',mode,sql,'--two-digit-year-max',last) == 'D\n'+date_serial(year,2,29,last)+'\n'
                controls += 1
        for sql, expected in [('SELECT DateSerial(2024,CDec(2.5),28.5) AS D', date_serial(2024,2,28)), ('SELECT TimeSerial(0,1.5,0) AS D', time_serial(0,2,0))]:
            assert run(args.cli, 'query', mode, sql) == 'D\n'+expected+'\n'
            controls += 1
        for expression in ['DateSerial(NULL,1,1)', 'TimeSerial(NULL,0,0)', 'DateSerial(100,1,0)', 'DateSerial(9999,12,32)', 'DateSerial(32767,32767,32767)', 'TimeSerial(32768,0,0)', 'DateSerial(2024,,1)', 'TimeSerial(1,2)', 'DateSerial(NULL,1,1/0)']:
            result = subprocess.run([str(args.cli),'query',mode,'SELECT 1 AS Before,'+expression+' AS D'], capture_output=True)
            assert result.returncode != 0 and result.stdout == b'', (mode, expression)
            controls += 1
    for mode in ['eval', 'run']:
        for expression, error in [
            ('DateSerial(CVErr(1),1/0,1)', 'division by zero'),
            ('TimeSerial(CVErr(1),0,1/0)', 'division by zero'),
            ('DateSerial(NULL,CVErr(1),1)', 'NULL'),
            ('DateSerial(CVErr(1),NULL,1)', 'type mismatch'),
            ('DateSerial(32768,CVErr(1),1)', 'overflow'),
            ('DateSerial(9999,13,CVErr(1))', 'type mismatch'),
        ]:
            result = subprocess.run([str(args.cli), 'query', mode, 'SELECT 1,' + expression], capture_output=True, text=True)
            assert result.returncode == 2 and not result.stdout and error in result.stderr, (expression, result)
            controls += 1
        assert run(args.cli, 'query', mode, 'SELECT DateSerial(CLng(CVErr(2001)),1,1) AS D') == 'D\n' + date_serial(2001, 1, 1) + '\n'
        controls += 1
    if args.native_directory:
        options = ['--index', 'IndexedRows', 'IdAsc', '--native-bytes', '67108864',
                   '--native-rows', '100000', '--native-steps', '10000000', '--execution-steps', '10000000']
        for suffix, expected_digest in INPUT_HASHES.items():
            database = args.native_directory / ('index-' + suffix)
            assert digest(database) == expected_digest
            for expression, expected in [
                ('DateSerial(2000+ID,1,1)', date_serial(2001, 1, 1)),
                ('TimeSerial(ID,2,3)', time_serial(1, 2, 3)),
                ('DateSerial(CLng(CVErr(ID))+2000,1,1)', date_serial(2001, 1, 1)),
            ]:
                sql = 'SELECT ' + expression + ' AS D FROM IndexedRows WHERE ID=1'
                assert run(args.cli, 'query', 'run', sql, '--database', database, *options) == 'D\n' + expected + '\n'
                controls += 1
            for expression, error in [
                ('DateSerial(CVErr(ID),1/(ID-1),1)', 'division by zero'),
                ('TimeSerial(CVErr(ID),0,1/(ID-1))', 'division by zero'),
                ('DateSerial(NULL,CVErr(ID),1)', 'NULL'),
                ('DateSerial(9999,13,CVErr(ID))', 'type mismatch'),
            ]:
                sql = 'SELECT 1,' + expression + ' FROM IndexedRows WHERE ID=1'
                result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(database), *options], capture_output=True, text=True)
                assert result.returncode == 2 and not result.stdout and error in result.stderr, (expression, result)
                controls += 1
            assert digest(database) == expected_digest
    if args.database:
        before = digest(args.database)
        tagged = run('java','-Dstdout.encoding=UTF-8','-cp',args.jar,RECIPES/'TableDataOracle.java',args.database,'ROULETTE_TB')
        ids = sorted(int(line.split('\t')[1].removeprefix('I32:')) for line in tagged.splitlines())
        sql = 'SELECT ORDR_NBR AS N,DateSerial(2000+ORDR_NBR,2,29) AS D FROM ROULETTE_TB ORDER BY N'
        expected = 'N\tD\n'+''.join(str(n)+'\t'+date_serial(2000+n,2,29)+'\n' for n in ids)
        for options in [[],['--index','ROULETTE_TB','Number']]:
            assert run(args.cli,'query','run',sql,'--database',args.database,*options) == expected
            controls += 1
        sql='SELECT ORDR_NBR AS N,DateSerial(ORDR_NBR,2,29) AS D FROM ROULETTE_TB ORDER BY N'
        for last in [2029,2049]:
            expected='N\tD\n'+''.join(str(n)+'\t'+date_serial(n,2,29,last)+'\n' for n in ids)
            assert run(args.cli,'query','run',sql,'--database',args.database,'--two-digit-year-max',last) == expected
            controls += 1
        for last in [2029,2049]:
            chosen=int(date_serial(30,1,1,last)[1:5])-2000
            expected='N\n'+''.join(str(n)+'\n' for n in ids if n == chosen)
            sql='SELECT ORDR_NBR AS N FROM ROULETTE_TB WHERE ORDR_NBR=Year(DateSerial(30,1,1))-2000 ORDER BY N'
            assert run(args.cli,'query','run',sql,'--database',args.database,'--index','ROULETTE_TB','Number','--two-digit-year-max',last) == expected
            controls += 1
        assert digest(args.database) == before
    print(f'Passed {controls} Gregorian constructor model controls; native Access unrun.')



if __name__ == '__main__':
    main()
