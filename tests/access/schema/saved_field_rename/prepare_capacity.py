"""Freeze a natural retained-history fitting boundary without production codecs."""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json

import fitting
from engineering_paths import repository_root

ROOT = repository_root(__file__)
ORIGINAL_SHA256 = 'a6a9e18cd35dc2bf018e5cd554a68b5b60d56023fd78a4a0ecba627ab9041d72'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--number-first', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    original = args.number_first.read_bytes()
    assert digest(original) == ORIGINAL_SHA256
    owner_path = ROOT / 'tests/access/schema/number_column_rollover/model.py'
    owner = load('fitting_capacity_number_history', owner_path)
    off_path = ROOT / 'tests/access/schema/nac_after_field_add/model.py'
    off = load('fitting_capacity_off_role', off_path)
    second_name = 'SecondNumberColumnXX'
    assert len(second_name) == 20
    before = owner.append_same_page(original, second_name)
    for number in range(3, 13):
        before = owner.append_same_page(before, 'N' + str(number))
    before, _ = off.expected(before, ROOT)
    free = fitting.u16(before, fitting.PROPERTY * fitting.PAGE + fitting.FREE)
    rows = fitting.u16(before, fitting.PROPERTY * fitting.PAGE + 12)
    assert free == 46 and rows == 13
    maximum_units = 5 + free // 2
    assert maximum_units == 28
    after, facts = fitting.rename(before, 'Score', 'R' * maximum_units)
    assert fitting.u16(after, fitting.PROPERTY * fitting.PAGE + fitting.FREE) == 0
    args.output_dir.mkdir(exist_ok=False)
    (args.output_dir / 'before.accdb').write_bytes(before)
    (args.output_dir / 'maximum28.accdb').write_bytes(after)
    report = {
        'scope': 'Independent natural Number history capacity prediction; no SDK/native writer execution',
        'source_number_first_sha256': ORIGINAL_SHA256,
        'source_second_field': second_name,
        'retained_number_fields': 12,
        'lval_rows_including_off': rows,
        'free_before': free,
        'original_units': 5,
        'maximum_units': maximum_units,
        'first_nonfitting_units': maximum_units + 1,
        'replacement': 'R' * maximum_units,
        'before_sha256': digest(before),
        'after_sha256': digest(after),
        'after_free': 0,
        'file_bytes': len(before),
        'tool_pins': {str(path.relative_to(ROOT)): digest(path.read_bytes()) for path in [
            owner_path, off_path,
            ROOT / 'tests/access/schema/number_column_repeat/model.py',
            ROOT / 'tests/access/schema/number_column_repeat/raw.py',
            ROOT / 'tests/access/schema/ace16_database_options/model.py',
            Path(__file__).resolve(),
            Path(fitting.__file__).resolve(),
        ]},
        'framing': facts,
    }
    (args.output_dir / 'prediction.json').write_text(json.dumps(report, indent=2) + '\n')
    assert args.number_first.read_bytes() == original
    print(json.dumps({key: report[key] for key in [
        'before_sha256', 'after_sha256', 'maximum_units', 'first_nonfitting_units', 'free_before',
    ]}))


if __name__ == '__main__':
    main()
