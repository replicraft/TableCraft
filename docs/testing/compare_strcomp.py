"""Compare the explicit StrComp profiles to independent string/UTF-16 models.

The database-ordinal controls use the CLI's selected ordinal policy; they are not
native Access locale/case/code-page comparisons. No host text locale is inferred.
"""
import argparse
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, run
from compare_large_tables import digest


def quoted(value):
    return "'"+value.replace("'", "''")+"'"


def sign(left, right):
    return (left > right)-(left < right)


def binary(left, right):
    return sign(left.encode('utf-16-be'), right.encode('utf-16-be'))


def refused(cli, mode, sql, options=()):
    result = subprocess.run([str(cli),'query',mode,sql,*options], capture_output=True)
    assert result.returncode != 0 and result.stdout == b'', (mode,sql)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--database', type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    before = digest(args.database)
    pairs = [('', ''),('', 'a'),('a', ''),('ABCD','abcd'),('abcd','ABCD'),('a','a '),('é','e\u0301'),('😀','\ue000'),('O\'Reilly',"O'Reilly"),('中','é')]
    controls = 0
    for mode in ['eval','run']:
        for left,right in pairs:
            sql=f'SELECT StrComp({quoted(left)},{quoted(right)},0) AS C'
            assert run(args.cli,'query',mode,sql) == f'C\n{binary(left,right)}\n'
            controls += 1
        for expression in ["StrComp(NULL,'x',0)","StrComp('x',NULL,1)","StrComp(NULL,NULL)"]:
            assert run(args.cli,'query',mode,'SELECT '+expression+' AS C') == 'C\nNULL\n'
            controls += 1
        for expression in ["StrComp('a','b')","StrComp('a','b',1)","StrComp('a','b',2)","StrComp(NULL,'b',NULL)","StrComp(NULL,'b',1/0)","StrComp('a','a',3)","StrComp(1,'a',0)","StrComp('a',,0)"]:
            refused(args.cli,mode,'SELECT 1 AS Before,'+expression+' AS C')
            controls += 1
    for left,right in pairs:
        for comparison in ['',',-1',',2',',']:
            sql=f'SELECT StrComp({quoted(left)},{quoted(right)}{comparison}) AS C'
            assert run(args.cli,'query','run',sql,'--collation','ordinal') == f'C\n{sign(left,right)}\n'
            controls += 1
    refused(args.cli,'run',"SELECT 1,StrComp('A','a',1)",['--collation','ordinal'])
    controls += 1
    tagged = run('java','-Dstdout.encoding=UTF-8','-cp',args.jar,RECIPES/'TableDataOracle.java',args.database,'ROULETTE_TB')
    records = [line.split('\t')[1:] for line in tagged.splitlines()]
    rows=[]
    for row in records:
        identity=int(row[0].removeprefix('I32:'))
        value=None if row[1]=='N' else bytes.fromhex(row[1].removeprefix('T')).decode('utf-8')
        rows.append((identity,'NULL' if value is None else str(binary(value,'20'))))
    expected='N\tC\n'+''.join(f'{n}\t{result}\n' for n,result in sorted(rows))
    for options in [[],['--index','ROULETTE_TB','Number']]:
        sql="SELECT ORDR_NBR AS N,StrComp(NBR_VALUE,'20',0) AS C FROM ROULETTE_TB ORDER BY N"
        assert run(args.cli,'query','run',sql,'--database',args.database,*options) == expected
        controls += 1
    assert digest(args.database) == before
    print(f'Passed {controls} StrComp UTF-16/ordinal/NULL/refusal controls; unchanged source {before}; native Access unrun.')


if __name__ == '__main__':
    main()
