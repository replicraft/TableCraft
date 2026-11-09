"""Freeze every complete independent prediction before any SDK invocation."""
from pathlib import Path
import argparse
import hashlib
import json
import shutil
import model

REPO = Path.cwd()
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--sources-dir', type=Path, required=True)
parser.add_argument('--output-dir', type=Path, required=True)
args = parser.parse_args()
ROOT = args.output_dir
ROOT.mkdir(exist_ok=False)
MODEL_SOURCE = Path(__file__).with_name('model.py')
SOURCES = [
    ('number-first-long', 'fbcc7a8ab6fa4d10df64c376c80b99ca76d1200d64d8d4a5d31d7b3702cc7680'),
    ('text-number-text-text', 'd5940a703ae582f27958b4369c8cdca97d9f566dd7cb79c63c4a39e72edf4002'),
]
OWNERS = [
    'tests/access/schema/field_add_after_off/model.py',
    'tests/access/rows/short_text_add/model.py',
    'tests/access/schema/number_column_repeat/raw.py',
    'tests/access/schema/fields/CreatedNumberColumnOracle.java',
    'tests/access/rows/short_text_add/CreatedShortTextColumnOracle.java',
]


def digest(data):
    return hashlib.sha256(data).hexdigest()


assert not (ROOT / 'predictions-before-sdk.json').exists()
for directory in ['inputs', 'expected', 'outputs', 'classes']:
    (ROOT / directory).mkdir(exist_ok=False)
report = {'date': '2026-11-09', 'scope': 'Independent second-ADD-after-OFF predictions frozen before SDK mutation',
          'model_sha256': digest(MODEL_SOURCE.read_bytes()),
          'owners': [{'source': name, 'sha256': digest((REPO / name).read_bytes())} for name in OWNERS],
          'cases': []}
for label, pin in SOURCES:
    original = args.sources_dir / (label + '.accdb')
    before = original.read_bytes()
    assert digest(before) == pin
    for kind, column in [('LONG', 'SecondNumber'), ('TEXT', 'SecondLabel')]:
        name = label + '-' + kind.lower()
        source = ROOT / 'inputs' / (name + '.accdb')
        shutil.copyfile(original, source)
        expected = model.prediction(before, kind, column, REPO)
        (ROOT / 'expected' / source.name).write_bytes(expected)
        report['cases'].append({'case': name, 'source': original.name,
                                'input': 'inputs/' + source.name, 'source_sha256': pin,
                                'kind': kind, 'column': column, 'expected_sha256': digest(expected),
                                'expected_bytes': len(expected),
                                'prediction_facts': model.observations(before, expected, REPO)})
(ROOT / 'predictions-before-sdk.json').write_text(json.dumps(report, indent=2) + '\n')
print('FROZEN', len(report['cases']), 'complete predictions')
