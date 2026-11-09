"""Run the unchanged first-party Number SDK helper on pinned prior AFTER copies."""
from pathlib import Path
import argparse
import hashlib
import json
import subprocess

HELPER = Path('tests/access/schema/fields/CreatedNumberColumnOracle.java')
JAR_HASH = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
HELPER_HASH = 'c805c038da5f0853c50a3b76753dab0577aa66322fe873c9e642e52a943d1886'
CASES = [
    ('id-only', 'Table1', 'Score2', 'a6a9e18cd35dc2bf018e5cd554a68b5b60d56023fd78a4a0ecba627ab9041d72'),
    ('gui-number-inline', 'Table1', 'Extra2', 'b314c02b5f67c8deebb564f7e4deae5cd5a5e28e1f67f3122b1cb8d14507c3a4'),
    ('existing-default-only', 'Items', 'Extra2', '05dcae27ffd63cba552e47740c60649d46bb265730b6b202a384e69ebb249072'),
]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def execute(command, label, root):
    result = subprocess.run(command, capture_output=True, text=True)
    (root / (label + '.stdout')).write_text(result.stdout)
    (root / (label + '.stderr')).write_text(result.stderr)
    if result.returncode:
        raise RuntimeError(f'{label} failed with exit {result.returncode}; retained logs')
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--sources-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    root = args.output_dir
    root.mkdir(exist_ok=False)
    assert digest(args.jar.read_bytes()) == JAR_HASH
    assert digest(HELPER.read_bytes()) == HELPER_HASH
    classes = root / 'classes'
    classes.mkdir(exist_ok=False)
    execute(['javac', '-cp', str(args.jar), '-d', str(classes), str(HELPER)], 'compile', root)
    observations = []
    for name, table, column, source_hash in CASES:
        source = args.sources_dir / (name + '-number-sdk-after.accdb')
        before = source.read_bytes()
        assert digest(before) == source_hash
        output = root / (name + '-second-number-sdk-after.accdb')
        assert not output.exists()
        stdout = execute([
            'java', '-cp', str(classes) + ':' + str(args.jar),
            'CreatedNumberColumnOracle', str(source), str(output),
            table, column, '--default-zero',
        ], name, root)
        assert source.read_bytes() == before
        after = output.read_bytes()
        observations.append({
            'case': name,
            'source': source.name,
            'output': output.name,
            'table': table,
            'column': column,
            'before_sha256': source_hash,
            'after_sha256': digest(after),
            'before_bytes': len(before),
            'after_bytes': len(after),
            'original_unchanged': True,
            'sdk_stdout': stdout.splitlines(),
        })
    report = {
        'scope': 'Independent repeated explicit Required(false)/DefaultValue0 Number ADD on prior SDK AFTER copies; no native candidate or Access execution',
        'sdk': {'artifact': 'Jackcess5.0.4', 'sha256': JAR_HASH, 'license': 'Apache-2.0'},
        'helper': {'source': str(HELPER), 'sha256': HELPER_HASH, 'unchanged': True},
        'expression_evaluation': False,
        'observations': observations,
    }
    (root / 'sdk-observations.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(observations, indent=2))


if __name__ == '__main__':
    main()
