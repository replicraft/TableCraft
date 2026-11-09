"""Freeze independent raw DROP hypotheses; never invoke an SDK or native writer."""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import model

HERE = Path(__file__).resolve().parent
SHARED = ['docs/testing/saved_field_rename/model.py',
          'docs/testing/short_text_add/model.py']


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--sources-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(exist_ok=False)
    files = args.output_dir / 'files'
    files.mkdir()
    manifest = json.loads((HERE / 'source_manifest.json').read_text())
    cases = []
    # Retain each attempted prediction immediately; a later failure cannot erase
    # already-frozen hypotheses or overwrite an earlier experiment directory.
    report = {'scope': 'Raw DROP hypotheses plus later inert reads only; no native Save claim',
              'frozen_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'local_helpers': {name: digest((HERE / name).read_bytes()) for name in
                                ['model.py', 'prepare.py', 'observe.py', 'FieldDropObserver.java',
                                 'source_manifest.json']},
              'shared_helpers': {name: digest((args.repo / name).read_bytes()) for name in SHARED},
              'cases': cases}
    for source in manifest['sources']:
        original = args.sources_dir / source['source']
        before = original.read_bytes()
        assert digest(before) == source['before_sha256']
        (files / (source['case'] + '-before.accdb')).write_bytes(before)
        for variant in ['td-only', 'append-live-group-removed']:
            item = {**source, 'variant': variant}
            cases.append(item)
            try:
                after, facts = model.prediction(before, source['drop'], variant, args.repo)
                filename = source['case'] + '-' + variant + '.accdb'
                (files / filename).write_bytes(after)
                item.update({'after': 'files/' + filename, 'after_sha256': digest(after), **facts})
            except Exception as error:
                item['prediction_failure'] = type(error).__name__ + ': ' + str(error)
                raise
            finally:
                assert original.read_bytes() == before
                (args.output_dir / 'predictions-before-observation.json').write_text(
                    json.dumps(report, indent=2) + '\n')
    print('Frozen', len(cases), 'raw hypotheses; no SDK command executed')


if __name__ == '__main__':
    main()
