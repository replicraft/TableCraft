"""First-party inert SDK copies for explicit Short Text and mixed field additions."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import subprocess

AREA = Path('docs/testing/short_text_add')
NUMBER_HELPER = Path('docs/testing/CreatedNumberColumnOracle.java')
TEXT_HELPER = AREA / 'CreatedShortTextColumnOracle.java'
PROPERTY_HELPER = AREA / 'ShortTextProperties.java'
JAR_HASH = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
HELPERS = [
    (NUMBER_HELPER, 'c805c038da5f0853c50a3b76753dab0577aa66322fe873c9e642e52a943d1886'),
    (TEXT_HELPER, '74b09b0ae88486dcf395d85c8fd4b4e454900c80e256c3a888e3894a3ac7f19e'),
    (PROPERTY_HELPER, 'c471fabb0be3cbf2986bdfcd938c98d223a9088754dbe12234f07fe4fb6b35ad'),
]
SOURCES = [
    ('id-only', 'generated-primary-before.accdb', 'b52d124892cdaab8fbe9c5028c876f57f1f82ee280cea7984aad68f1ead5eb13'),
    ('inline-number', 'gui-number-inline-before.accdb', '9bb835b86e8e0789728834ae8871ab9b51d647c69bea89a091d15fd14cd8a90e'),
    ('two-number', 'id-only-second-number-sdk-after.accdb', '8ca406b45272b6225d8f44b37ba74336250257cb5b37f0d074200d1837ef9ff9'),
]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def execute(command, label, root):
    result = subprocess.run(command, capture_output=True, text=True)
    (root / (label + '.stdout')).write_text(result.stdout)
    (root / (label + '.stderr')).write_text(result.stderr)
    if result.returncode:
        raise RuntimeError(f'{label}: exit {result.returncode}; failure logs retained')
    return result.stdout.splitlines()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--sources-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar.read_bytes()) == JAR_HASH
    for helper, pin in HELPERS:
        assert digest(helper.read_bytes()) == pin
    root = args.output_dir
    root.mkdir(exist_ok=False)
    files = root / 'files'
    files.mkdir()
    classes = root / 'classes'
    execute(['javac', '-cp', str(args.jar), '-d', str(classes),
             *[str(helper) for helper, _ in HELPERS]], 'compile', root)
    classpath = str(classes) + os.pathsep + str(args.jar)

    def java(helper, values, label):
        return execute(['java', '-Xmx256m', '-cp', classpath, helper, *values], label, root)

    def added(source, column, label, declaration, source_location):
        before = source.read_bytes()
        output = files / (label + '-after.accdb')
        assert not output.exists()
        if declaration['type'] == 'LONG':
            stdout = java('CreatedNumberColumnOracle',
                          [str(source), str(output), 'Table1', column, '--default-zero'], label)
            getter_column = 'Label'
            compressed = 'true'
        else:
            compressed = str(declaration['unicode_compression']).lower()
            stdout = java('CreatedShortTextColumnOracle',
                          [str(source), str(output), 'Table1', column, str(declaration['units']), compressed], label)
            getter_column = column
        assert source.read_bytes() == before
        after = output.read_bytes()
        if declaration['type'] == 'TEXT' and declaration['units'] != 255:
            units = declaration['units']
            assert any(line.startswith(f'Column\t{column}\tTEXT\tunits={units}\t') for line in stdout)
            metadata = {'width_getter_observed': True,
                        'property_presence': 'The exact MR2 body is independently compared; the separate255observer is not used.'}
        else:
            metadata = java('ShortTextProperties',
                            [str(output), 'Table1', getter_column, compressed], label + '-properties')
        assert output.read_bytes() == after
        return {
            'case': label, 'column': column,
            'source': source.name, 'source_location': source_location, 'output': output.name,
            'source_sha256': digest(before), 'after_sha256': digest(after),
            'source_bytes': len(before), 'after_bytes': len(after),
            'declaration': declaration, 'original_unchanged': True,
            'stdout': stdout, 'typed_read_only_properties': metadata,
        }

    sources = []
    for name, filename, pin in SOURCES:
        source = args.sources_dir / filename
        assert digest(source.read_bytes()) == pin
        sources.append((name, source, 'inputs'))
    before_third = sources[2][1]
    third = files / 'third-number-sdk-after.accdb'
    third_stdout = java('CreatedNumberColumnOracle',
                        [str(before_third), str(third), 'Table1', 'Third', '--default-zero'],
                        'third-number')
    assert digest(before_third.read_bytes()) == SOURCES[2][2]
    sources.append(('three-number', third, 'sdk_files'))

    observations = []
    for name, source, location in sources:
        for compressed in [False, True]:
            label = name + ('-compressed' if compressed else '-uncompressed')
            declaration = {'type': 'TEXT', 'units': 255, 'required': False,
                           'allow_zero_length': True, 'default': 'absent',
                           'unicode_compression': compressed}
            observations.append(added(source, 'Label', label, declaration, location))
    for source in observations[:8]:
        if not source['declaration']['unicode_compression']:
            continue
        before = files / source['output']
        assert digest(before.read_bytes()) == source['after_sha256']
        text = dict(source['declaration'])
        observations.append(added(before, 'OtherLabel', 'repeat-label-on-' + source['case'], text, 'sdk_files'))
        number = {'type': 'LONG', 'required': False, 'default': 'typed Int32 zero'}
        observations.append(added(before, 'AfterLabel', 'number-after-label-on-' + source['case'], number, 'sdk_files'))

    for units in [1, 80]:
        declaration = {'type': 'TEXT', 'units': units, 'required': False,
                       'allow_zero_length': True, 'default': 'absent',
                       'unicode_compression': True}
        observations.append(added(sources[0][1], 'Label', f'id-only-width{units}-compressed',
                                  declaration, 'inputs'))

    report = {
        'scope': 'Explicit inert SDK declaration/mixed-source evidence; no native writer or Access execution',
        'sdk': {'artifact': 'Jackcess5.0.4', 'sha256': JAR_HASH, 'license': 'Apache-2.0'},
        'helpers': [{'source': str(helper), 'sha256': pin} for helper, pin in HELPERS],
        'third_number': {'source_sha256': SOURCES[2][2], 'after_sha256': digest(third.read_bytes()),
                         'stdout': third_stdout},
        'observations': observations,
    }
    (root / 'sdk-observations.json').write_text(json.dumps(report, indent=2) + '\n')
    print('SDK copies and typed metadata complete:', len(observations), '+ third Number')


if __name__ == '__main__':
    main()
