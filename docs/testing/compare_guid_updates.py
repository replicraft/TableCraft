"""Compare literal GUID update closures, independent cursors and authorized spans."""
import argparse
from pathlib import Path
from compare_batch_updates import Cell, compare

A="00112233-4455-6677-8899-aabbccddeeff"
B="00112233-4455-6677-8899-aabbccddee00"
NEW="fedcba98-7654-3210-ffff-000011112222"
NEAR="00112233-4455-6677-8899-aabbccddee80"

def g(table,row,old,new):
    return Cell(table,"G",row,"guid",old,new)

def n(table,row,old,new):
    return Cell(table,"N",row,"int64",old,new)

CASES=(
    (g("Tree",1,B,NEAR),),
    (g("Tree",0,None,NEAR),),
    (g("Tree",2,A,None),),
    (g("Tree",998,A,NEAR),),
    (g("Unique",0,A,B),g("Unique",1,B,A)),
    (g("Unique",0,A,None),),
    (g("Unique",2,None,NEW),),
    (g("Unique",0,A,A),),
    (g("Parents",0,A,NEW),g("Children",0,A,NEW),g("Children",4,A,NEW)),
    (g("TupleParents",0,A,NEW),n("TupleParents",0,0,9007199254740993),
     g("TupleChildren",0,A,NEW),n("TupleChildren",0,0,9007199254740993),
     g("TupleChildren",4,A,NEW),n("TupleChildren",4,0,9007199254740993)),
    (g("TupleParents",0,A,None),n("TupleParents",0,0,None),
     g("TupleChildren",0,A,None),n("TupleChildren",0,0,None),g("TupleChildren",4,A,None),n("TupleChildren",4,0,None)),
    (g("Parents",0,A,B),g("Parents",1,B,A),g("Children",0,A,B),g("Children",1,B,A),g("Children",4,A,B)),
)
INVALID=(
    ((g("Tree",1,B,NEW),),b"IndexedColumn"),
    ((g("Unique",0,A,B),),b"DuplicateKey"),
    ((g("Children",0,A,NEW),),b"MissingParent"),
    ((g("Unique",0,B,NEW),),b"Conflict"),
    ((g("Unique",0,A,NEW),g("Unique",0,A,NEW)),b"Conflict"),
)

def request(cells):
    parents=tuple(c for c in cells if c.table in ("Parents","TupleParents"))
    return parents or cells

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ("jar","cli","rows","preview","checker","ace2016","ace2019"):
        parser.add_argument("--"+name,required=True,type=Path)
    args=parser.parse_args()
    compare(args,((args.ace2016,5,CASES,INVALID),(args.ace2019,6,CASES,INVALID)),request=request,cascade=True)

if __name__=="__main__":
    main()
