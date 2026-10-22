"""Original independent encounter-selection controls for expression Error data.

List endpoints model selection; native getters provide source encounter IDs.
No Microsoft Access aggregate/Variant evaluation is performed.
"""

import argparse
import hashlib
from pathlib import Path
import random

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256, execute


def cell(value):
    if value is None or value[0] == 0:
        return "NULL"
    kind, code = value
    return f"#ERROR({code})" if kind == 1 else str(code)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261021)
    count = 0
    expression = "IIf(Kind=0,Null,IIf(Kind=1,CVErr(Code),Code))"
    cases = [[], [(0, 0)], [(1, 0)], [(2, 65535)]]
    cases += [[(rng.randrange(3), rng.randrange(65536)) for _ in range(rng.randrange(32))] for _ in range(252)]
    for values in cases:
        source = " UNION ALL ".join(f"SELECT {kind} AS Kind,{code} AS Code" for kind, code in values)
        if not values:
            source = "SELECT 0 AS Kind,0 AS Code WHERE False"
        first = values[0] if values else None
        last = values[-1] if values else None
        sql = f"SELECT First({expression}),Last({expression}),IsError(First({expression})),IsNull(Last({expression})) FROM ({source}) AS Q"
        expected = [cell(first), cell(last), "TRUE" if first and first[0] == 1 else "FALSE", "TRUE" if last is None or last[0] == 0 else "FALSE"]
        assert execute(args.cli, "run", sql) == [expected]
        count += len(expected)
    for aggregate in ["First", "Last"]:
        sql = f"SELECT {aggregate}(IIf(Code=1,CVErr(Code),1/(2-Code))) FROM (SELECT 1 AS Code UNION ALL SELECT 2) AS Q"
        execute(args.cli, "run", sql, success=False)
        count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = [int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines()]
        first, last = identities[0], identities[-1]
        sql = f"SELECT First(CVErr(ID)),Last(CVErr(ID)),DFirst('CVErr(ID)','ScalarSamples'),DLast('CVErr(ID)','ScalarSamples'),DLookup('CVErr(ID)','ScalarSamples','ID={first}'),IsError(First(CVErr(ID))) FROM ScalarSamples"
        assert execute(args.cli, "run", sql, database) == [[f"#ERROR({value})" for value in [first, last, first, last, first]] + ["TRUE"]]
        count += 6
        execute(args.cli, "run", f"SELECT First(IIf(ID={first},CVErr(ID),1/({last}-ID))) FROM ScalarSamples", database, success=False)
        execute(args.cli, "run", f"SELECT DFirst('IIf(ID={first},CVErr(ID),1/({last}-ID))','ScalarSamples')", database, success=False)
        count += 2
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent encounter/NULL/Error/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
