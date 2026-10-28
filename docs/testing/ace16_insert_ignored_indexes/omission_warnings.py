"""Exact observed JUL framing for the pinned library's ignored-NULL no-op."""
from pathlib import Path
import re
from ace16_insert_ignored_indexes import reference

HEADER = re.compile(r'(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) [0-9]{1,2}, 2026 [0-9]{1,2}:[0-9]{2}:[0-9]{2} (?:AM|PM) com\.healthmarketscience\.jackcess\.impl\.IndexData commitAddRow')


def check(stderr, count, database):
    lines = stderr.decode('utf-8', errors='strict').splitlines()
    assert len(lines) == 2 * count, ('Unexpected warning count', count, len(lines))
    wanted = f'WARNING: Added duplicate index entry null (Db={database};Table=Items;Index=[GroupOrder, GroupAlias])'
    for position in range(count):
        assert HEADER.fullmatch(lines[2 * position]), lines[2 * position]
        assert lines[2 * position + 1] == wanted, lines[2 * position + 1]
    return count


def check_command(command, case, stderr):
    phase = command['phase']
    if phase == 'generate':
        rows = reference.source_rows(case['variant'])
        database = Path(case['source']).name
    elif phase == 'reference-append':
        rows = reference.requests(case['variant'])
        database = Path(case['reference']).name
    else:
        assert not stderr
        return 0
    return check(stderr, sum(row[1] is None for row in rows), database)


