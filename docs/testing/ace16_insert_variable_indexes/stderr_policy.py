"""Exact known omitted-selected-NULL warnings, with no blanket stderr fallback."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_ignored_indexes import omission_warnings
from ace16_insert_variable_indexes import recipes


def check(command, case, stderr):
    phase = command['phase']
    if phase == 'generate':
        rows = recipes.source_rows(case)
        database = Path(case['source']).name
    elif phase == 'reference-append':
        rows = recipes.requests(case)
        database = Path(case['reference']).name
    else:
        assert stderr == b''
        return 0
    count = sum(not recipes.eligible(row, case) for row in rows)
    return omission_warnings.check(stderr, count, database)
