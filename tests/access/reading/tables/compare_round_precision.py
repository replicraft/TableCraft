"""Independent exact-binary/Decimal model for Round's extended precision counts."""
import argparse
import csv
from decimal import Decimal, ROUND_HALF_EVEN, localcontext
from io import StringIO
from pathlib import Path
import math
import random
import struct
import subprocess


def bits(value):
    return struct.pack(">d", value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    samples = [
        0.0, -0.0, 1e-30, -1e-30, 2.0 ** -100,
        1e-100, -1e-300, 1e-308, 1e-309, 1e-323,
        float.fromhex("0x1.fffffffffffffp+51"), float.fromhex("0x1.fffffffffffffp+1023"),
        *[float.fromhex("0x0.000000000000" + str(value) + "p-1022") for value in range(1, 10)],
    ]
    generator = random.Random(20261024)
    while len(samples) < 64:
        value = struct.unpack(">d", generator.getrandbits(64).to_bytes(8, "big"))[0]
        if math.isfinite(value):
            samples.append(value)

    controls = 0
    with localcontext() as context:
        context.prec = 1200
        for value in samples:
            for places in [29, 30, 31, 45, 100, 200, 307, 308, 309, 310, 322, 323, 324, 32767, 2147483647]:
                expected = value if places >= 324 else float(
                    Decimal.from_float(value).quantize(Decimal(1).scaleb(-places), rounding=ROUND_HALF_EVEN)
                )
                result = subprocess.run(
                    [str(args.cli.resolve()), "query", "run",
                     "PARAMETERS P VALUE,N LONG; SELECT Round(P,N) AS R,VarType(Round(P,N)) AS T",
                     "--param", f"P=double:{value!r}", "--param", f"N=long:{places}"],
                    capture_output=True, text=True, encoding="utf-8", timeout=30, check=False,
                )
                if result.returncode or result.stderr:
                    raise RuntimeError((value, places, result.stderr))
                rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                if len(rows) != 2 or rows[0] != ["R", "T"] or rows[1][1] != "5":
                    raise RuntimeError("Incomplete Round result or incorrect carrier")
                if bits(float(rows[1][0])) != bits(expected):
                    raise RuntimeError((value, places, rows[1], expected))
                controls += 1
    print(f"Passed {controls} exact IEEE result/carrier controls, including subnormals and signed zero.")
    print("Native Access precision/count/error qualification remains unrun.")


if __name__ == "__main__":
    main()
