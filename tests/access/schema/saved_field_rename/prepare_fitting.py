"""Freeze fitted raw rename images and damaged controls before reader observations."""
from pathlib import Path
import argparse
import datetime
import hashlib
import json
import fitting

HERE = Path(__file__).resolve().parent


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sources-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(exist_ok=False)
    files = args.output_dir / 'files'
    files.mkdir()
    manifest = json.loads((HERE / 'source_manifest.json').read_text())
    cases = []
    for source in manifest['sources']:
        original = args.sources_dir / source['source']
        before = original.read_bytes()
        assert digest(before) == source['before_sha256']
        for new in ['TaxAmount', 'X']:
            direction = 'grow' if len(new) > len(source['old_name']) else 'shrink'
            for variant in ['coordinated', 'tdef-only', 'stale-descriptor', 'stale-user-row']:
                after, facts = fitting.rename(before, source['old_name'], new, variant)
                output = files / (source['case'] + '-' + direction + '-' + variant + '.accdb')
                output.write_bytes(after)
                cases.append({
                    'case': source['case'],
                    'old_name': source['old_name'],
                    'new_name': new,
                    'source': source['source'],
                    'before_sha256': digest(before),
                    'after': str(output.relative_to(args.output_dir)),
                    'after_sha256': digest(after),
                    'after_bytes': len(after),
                    **facts,
                })
        assert original.read_bytes() == before
    report = {
        'frozen_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'scope': 'Independent raw fitting hypotheses, no SDK or native Access write',
        'model_sha256': digest((HERE / 'fitting.py').read_bytes()),
        'baseline_model_sha256': digest((HERE / 'model.py').read_bytes()),
        'cases': cases,
    }
    (args.output_dir / 'predictions-before-observation.json').write_text(json.dumps(report, indent=2) + '\n')
    print('FROZEN', len(cases), 'raw fitting images and controls before any reader observation')


if __name__ == '__main__':
    main()
