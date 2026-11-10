"""Pin sequential renames of actual current images, retaining historical slack."""
from pathlib import Path
import argparse
import json

import fitting
from prepare_populated import SOURCE_SHA256, digest, long_rows


NAMES = ['Score', 'TaxAmount', 'X', 'Revenue', 'Score']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--number-first-off', type=Path, required=True)
    parser.add_argument('--number-history-off', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    first = args.number_first_off.read_bytes()
    history = args.number_history_off.read_bytes()
    assert digest(first) == SOURCE_SHA256
    assert digest(history) == '6f9b095ddf3d724e34d5af98bea4e8ba88bc2b2bc70ade10814f85c27770c620'
    args.output_dir.mkdir(exist_ok=False)
    cases = []
    for label, original in [('empty', first), ('history', history),
                            ('populated', long_rows(first, [42, None, -7]))]:
        current = original
        for index, (old, new) in enumerate(zip(NAMES, NAMES[1:]), 1):
            before = current
            current, _ = fitting.rename(before, old, new)
            assert current[8 * 4096:10 * 4096] == original[8 * 4096:10 * 4096]
            (args.output_dir / f'{label}-{index}-{new}.accdb').write_bytes(current)
            cases.append({'case': label, 'step': index, 'old_name': old, 'new_name': new,
                          'before_sha256': digest(before), 'after_sha256': digest(current),
                          'rows_and_keys_unchanged': True})
        assert current != original, 'Returning a name must retain genuine free-space history'
    report = {'scope': 'Independent raw sequential transforms; no SDK/native writer execution',
              'cases': cases}
    (args.output_dir / 'predictions.json').write_text(json.dumps(report, indent=2) + '\n')
    assert args.number_first_off.read_bytes() == first
    assert args.number_history_off.read_bytes() == history
    print(json.dumps(cases, indent=2))


if __name__ == '__main__':
    main()
