"""Observe a third Text save after the retained Text/Number SDK sequence."""
from pathlib import Path
import argparse
import hashlib
import json
import os

import model
import probe

RECORD = Path('docs/testing/results/add-short-text-sdk-source-2026-11-08.json')
COLUMN = 'OtherLabel'
UNITS = 255


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--sources-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    if digest(args.jar.read_bytes()) != probe.JAR_HASH:
        raise ValueError('SDK artifact hash differs')
    for helper, pin in probe.HELPERS:
        if digest(helper.read_bytes()) != pin:
            raise ValueError('Maintained helper hash differs')

    observations = json.loads(RECORD.read_text())['observations']
    sources = [entry for entry in observations if entry['case'].startswith('number-after-label-on-')]
    if len(sources) != 4:
        raise ValueError('Expected four previously qualified mixed sources')
    originals = []
    raw = model.prior_raw()
    for entry in sources:
        path = args.sources_dir / entry['output']
        before = path.read_bytes()
        if digest(before) != entry['after_sha256']:
            raise ValueError('Retained SDK source hash differs')
        # Construct the entire expected image before any SDK mutation runs.
        expected = bytes(model.short_text(before, COLUMN, UNITS, True, raw))
        originals.append((entry, path, before, expected))

    root = args.output_dir
    root.mkdir(exist_ok=False)
    files = root / 'files'
    files.mkdir()
    classes = root / 'classes'
    probe.execute(
        ['javac', '-cp', str(args.jar), '-d', str(classes),
         str(probe.TEXT_HELPER), str(probe.PROPERTY_HELPER)],
        'compile', root,
    )
    classpath = str(classes) + os.pathsep + str(args.jar)
    results = []
    for entry, source, before, expected in originals:
        label = 'text-after-' + entry['case']
        output = files / (label + '-after.accdb')
        expected_path = files / (label + '-independent.accdb')
        expected_path.write_bytes(expected)
        stdout = probe.execute(
            ['java', '-Xmx256m', '-cp', classpath, 'CreatedShortTextColumnOracle',
             str(source), str(output), 'Table1', COLUMN, str(UNITS), 'true'],
            label, root,
        )
        after = output.read_bytes()
        if source.read_bytes() != before:
            raise ValueError('SDK changed the retained original')
        if after != expected:
            raise ValueError('Complete SDK AFTER differs from independent model')
        metadata = probe.execute(
            ['java', '-Xmx256m', '-cp', classpath, 'ShortTextProperties',
             str(output), 'Table1', COLUMN, 'true'],
            label + '-properties', root,
        )
        if output.read_bytes() != after:
            raise ValueError('Read-only observer changed the SDK AFTER')
        results.append({
            'case': label, 'source': source.name, 'output': output.name,
            'source_sha256': digest(before), 'after_sha256': digest(after),
            'independent_sha256': digest(expected),
            'source_bytes': len(before), 'after_bytes': len(after),
            'original_unchanged': True, 'complete_image_equal': True,
            'declaration': {'column': COLUMN, 'type': 'TEXT', 'units': UNITS,
                            'unicode_compression': True, 'required': False,
                            'allow_zero_length': True, 'default': 'absent'},
            'stdout': stdout, 'typed_read_only_properties': metadata,
        })
    report = {
        'scope': 'Third Text save after retained Text/Number SDK saves; no native Microsoft Access execution',
        'sdk': {'artifact': 'Jackcess5.0.4', 'sha256': probe.JAR_HASH, 'license': 'Apache-2.0'},
        'helpers': [{'source': str(path), 'sha256': pin} for path, pin in probe.HELPERS],
        'model_sha256': digest(Path(model.__file__).read_bytes()),
        'raw_model_sha256': digest(model.RAW_SOURCE.read_bytes()),
        'recipe_sha256': digest(Path(__file__).read_bytes()),
        'observations': results,
    }
    (root / 'observations.json').write_text(json.dumps(report, indent=2) + '\n')
    print('Complete independent third-step SDK images matched:', len(results))


if __name__ == '__main__':
    main()
