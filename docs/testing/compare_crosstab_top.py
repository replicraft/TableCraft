"""Independent nullable rank-threshold controls for crosstab TOP (not Access SQL)."""
import argparse
import csv
from io import StringIO
from pathlib import Path
import random
import subprocess


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli',required=True,type=Path)
    args=parser.parse_args()
    rng=random.Random(20261020)
    controls=0
    samples=[[None,None,None],[2,2,1],[-1,3,3,None],[0],[-3,-2,-1,0,1,2,3]]
    samples += [[rng.choice([None,-3,-2,-1,0,1,2,3]) for _ in range(rng.randrange(1,13))] for _ in range(32)]
    for values in samples:
        source=' UNION ALL '.join(f'SELECT {index} AS Id, '+('NULL' if value is None else str(value))+' AS Amount' for index,value in enumerate(values))
        for count,percent in [(1,False),(2,False),(99,False),(1,True),(34,True),(50,True),(100,True)]:
            for descending,secondary in [(True,False),(True,True),(False,False)]:
                sql=f"TRANSFORM Sum(Amount) SELECT TOP {count} {'PERCENT' if percent else ''} Id,Sum(Amount) AS Total FROM ({source}) AS src GROUP BY Id ORDER BY Total {'DESC' if descending else 'ASC'}{',Id' if secondary else ''} PIVOT 1"
                result=subprocess.run([str(args.cli),'query','run',sql],capture_output=True,text=True)
                assert result.returncode==0,(sql,result.stderr)
                def key(index):
                    value=values[index]
                    # Explicit current SQL NULL ordering: before numbers in ASC.
                    return (value is not None,0 if value is None else value)
                ordered=sorted(range(len(values)),key=key,reverse=descending)
                # Python reverse sorting is stable; use an explicit ascending ID
                # within equal score keys when checking the complete sort tuple.
                if secondary:
                    ordered=sorted(ordered,key=lambda index:index)
                    ordered=sorted(ordered,key=key,reverse=descending)
                desired=(len(values)*count+99)//100 if percent else count
                chosen=ordered[:desired]
                if desired<len(ordered) and not secondary:
                    threshold=key(ordered[desired-1])
                    chosen=[index for index in ordered if key(index)>=threshold] if descending else [index for index in ordered if key(index)<=threshold]
                expected=[['Id','Total','1'],*[[str(index),*(['NULL' if values[index] is None else str(values[index])]*2)] for index in chosen]]
                actual=list(csv.reader(StringIO(result.stdout),delimiter='\t'))
                assert actual==expected,(values,count,percent,descending,secondary,actual,expected)
                controls+=1
    for top in ['TOP 0','TOP 0 PERCENT','DISTINCT TOP 1','DISTINCTROW TOP 1']:
        sql=f'TRANSFORM Count(*) SELECT {top} Id FROM (SELECT 1 AS Id) AS src GROUP BY Id PIVOT 1'
        result=subprocess.run([str(args.cli),'query','run',sql],capture_output=True)
        assert result.returncode==2 and not result.stdout
        controls+=1
    sql='TRANSFORM Sum(Amount/(Id-2)) SELECT TOP 1 Id FROM (SELECT 1 AS Id,10 AS Amount UNION ALL SELECT 2,20 UNION ALL SELECT 3,30) AS src GROUP BY Id ORDER BY Id PIVOT 1'
    result=subprocess.run([str(args.cli),'query','run',sql],capture_output=True)
    assert result.returncode==2 and not result.stdout and b'division by zero' in result.stderr
    controls+=1
    print(f'Passed {controls} independent crosstab TOP rank/refusal controls; native Access unrun.')


if __name__=='__main__':
    main()
