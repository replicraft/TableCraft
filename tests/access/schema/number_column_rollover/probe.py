"""Run unchanged first-party Number SDK operations on a pinned fresh-copy chain."""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import os
import subprocess


def owner_model():
    path = Path(__file__).with_name('model.py')
    spec = importlib.util.spec_from_file_location('number_rollover_model', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


model = owner_model()
HELPER = model.ROOT / 'tests/access/schema/fields/CreatedNumberColumnOracle.java'
JAR_HASH = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
HELPER_HASH = 'c805c038da5f0853c50a3b76753dab0577aa66322fe873c9e642e52a943d1886'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def execute(command, label, root):
    result = subprocess.run(command, text=True, capture_output=True)
    (root / (label + '.stdout')).write_text(result.stdout)
    (root / (label + '.stderr')).write_text(result.stderr)
    if result.returncode:
        raise RuntimeError(f'{label} failed with exit {result.returncode}; logs retained')
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar.read_bytes()) == JAR_HASH
    assert digest(HELPER.read_bytes()) == HELPER_HASH
    original = args.source.read_bytes()
    assert digest(original) == model.SOURCE_HASH
    root = args.output_dir
    root.mkdir(exist_ok=False)
    classes = root / 'classes'
    classes.mkdir()
    execute(['javac', '-cp', str(args.jar), '-d', str(classes), str(HELPER)], 'compile', root)
    source = root / 'score-pinned-before.accdb'
    source.write_bytes(original)
    observations = []
    for number in range(2, 15):
        name = f'N{number}'
        before = source.read_bytes()
        expected = model.append_same_page(before, name) if number <= 13 else model.rollover(before, name)
        independent = root / f'n{number}-independent-after.accdb'
        independent.write_bytes(expected)
        after_path = root / f'n{number}-sdk-after.accdb'
        stdout = execute([
            'java', '-cp', str(classes) + os.pathsep + str(args.jar),
            'CreatedNumberColumnOracle', str(source), str(after_path),
            'Table1', name, '--default-zero',
        ], f'n{number}', root)
        actual = after_path.read_bytes()
        assert source.read_bytes() == before and args.source.read_bytes() == original
        observations.append({
            'column': name, 'source': source.name, 'output': after_path.name,
            'before_bytes': len(before), 'after_bytes': len(actual),
            'before_sha256': digest(before), 'after_sha256': digest(actual),
            'independent_after_sha256': digest(expected),
            'full_image_match': expected == actual, 'source_unchanged': True,
            'sdk_stdout': stdout.splitlines(),
        })
        (root / 'sdk-observations.json').write_text(json.dumps({
            'scope': 'Unchanged first-party SDK helper; explicit Required false/default0; no native Access execution',
            'sdk': {'artifact': 'Jackcess 5.0.4', 'sha256': JAR_HASH, 'license': 'Apache-2.0'},
            'helper': {'source': 'tests/access/schema/fields/CreatedNumberColumnOracle.java', 'sha256': HELPER_HASH},
            'driver_sha256': digest(Path(__file__).read_bytes()),
            'observations': observations,
        }, indent=2) + '\n')
        print(name, expected == actual, digest(actual), flush=True)
        if expected != actual:
            raise RuntimeError('Independent complete-image mismatch; all outputs retained')
        source = after_path


if __name__ == '__main__':
    main()
