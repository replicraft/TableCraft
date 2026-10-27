"""Static model/capture checks only; no JVM/native/database execution."""
import ast
import json
from pathlib import Path
import sys

BASE = Path(__file__).resolve().parent
REPO = BASE.parents[2]
sys.path.insert(0, str(REPO / 'docs/testing'))
import model

for path in BASE.glob('*.py'):
    ast.parse(path.read_text(), filename=str(path))

for profile, code in [('V2016', 5), ('V2019', 6)]:
    header = bytearray(66)
    header[:4] = bytes.fromhex('00010000')
    header[4:20] = b'Standard ACE DB\0'
    header[20] = code
    header[62:66] = bytes.fromhex('fb8abc4e')
    model.qualify_header(header, profile)
    header[62:66] = bytes(4)
    try:
        model.qualify_header(header, profile)
    except AssertionError:
        pass
    else:
        raise AssertionError('Raw zero does not represent decoded encoding-key zero')

sizes = {}
for table, (_, width, count, _, maximum) in model.KINDS.items():
    references = [(32 + row // 255) << 8 | row % 255 for row in range(count)]
    for descending in [False, True]:
        before = sorted(model.wire(0, row, width, width == 1, descending) for row in references)
        after = sorted(model.wire(maximum if position == 0 else 0, row,
                                  width, width == 1, descending)
                       for position, row in enumerate(references))
        assert model.size(before) <= model.CAPACITY
        assert model.size(after) > model.CAPACITY
        middle = count // 2
        assert model.size(after[:middle]) <= model.CAPACITY
        assert model.size(after[middle:]) <= model.CAPACITY
        sizes[f'{table}-{descending}'] = {
            'before_best_bytes': model.size(before), 'after_best_bytes': model.size(after),
            'half_best_bytes': [model.size(after[:middle]), model.size(after[middle:])],
        }

print(json.dumps({'static_only': True, 'no_database_or_process_actions': True,
                  'illustrative_literal_row_references': True, 'sizes': sizes}, indent=2))
