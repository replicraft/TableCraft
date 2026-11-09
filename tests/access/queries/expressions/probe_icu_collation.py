"""Build/run an isolated ICU ordering probe; does not edit the workspace lockfile.

A fresh output directory outside the repository retains the manifest, resolved
lockfile, Cargo metadata, full report, executable and source hash for review.
This is a candidate experiment, never an Access collation provider or test oracle.
"""
import argparse
import hashlib
import json
import os
import platform
import shutil
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
SOURCE = Path(__file__).with_suffix('.rs')
CANDIDATE = '2.3.1'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--target-dir', type=Path, help='optional external build cache')
    parser.add_argument('--offline', action='store_true')
    args = parser.parse_args()
    output = args.output_dir.resolve()
    if output.is_relative_to(ROOT):
        parser.error('Use a fresh output directory outside the repository')
    target = args.target_dir.resolve() if args.target_dir else output / 'target'
    if target.is_relative_to(ROOT):
        parser.error('Use an external target directory')
    output.mkdir(parents=True, exist_ok=False)
    environment = os.environ | {'CARGO_TARGET_DIR': str(target)}
    if args.offline:
        environment['CARGO_NET_OFFLINE'] = 'true'
    source = SOURCE.read_bytes()
    snapshot = output / 'probe.rs'
    snapshot.write_bytes(source)
    workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['dependencies']
    # Local and already selected versions retain their repository owner. The
    # candidate version is isolated here until a working consumer merits adoption.
    dependencies = {
        'icu_collator': {'version': '=' + CANDIDATE, 'default-features': False, 'features': ['compiled_data']},
        'icu_locale_core': workspace['icu_locale_core'],
        'clap': workspace['clap'],
        'tablecraft-ace': {'path': str(ROOT / workspace['tablecraft-ace']['path']), 'default-features': False},
        'tablecraft-core': {'path': str(ROOT / workspace['tablecraft-core']['path']), 'default-features': False},
    }
    manifest = '[workspace]\n[package]\nname = "tablecraft-icu-probe"\nversion = "0.0.0"\nedition = "2024"\n'
    manifest += '[[bin]]\nname = "tablecraft-icu-probe"\npath = ' + json.dumps(str(snapshot)) + '\n[dependencies]\n'
    for name, spec in dependencies.items():
        values = []
        for key, value in spec.items():
            values.append(key + ' = ' + json.dumps(value))
        manifest += name + ' = { ' + ', '.join(values) + ' }\n'
    path = output / 'Cargo.toml'
    path.write_text(manifest)
    (output / 'source.sha256').write_text(hashlib.sha256(source).hexdigest() + '\n')
    command = ['cargo', 'build', '--release', '--manifest-path', str(path)]
    subprocess.run(command, cwd=ROOT, env=environment, check=True)
    metadata = subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version', '1', '--manifest-path', str(path)], cwd=ROOT, env=environment)
    (output / 'metadata.json').write_bytes(metadata)
    executable = target / 'release' / ('tablecraft-icu-probe.exe' if os.name == 'nt' else 'tablecraft-icu-probe')
    retained = output / executable.name
    shutil.copy2(executable, retained)
    executable = retained
    report = subprocess.check_output([str(executable)], cwd=ROOT)
    (output / 'report.tsv').write_bytes(report)
    context = {
        'schema': 'tablecraft-icu-collation-probe-v1',
        'candidate': CANDIDATE,
        'source_sha256': hashlib.sha256(source).hexdigest(),
        'binary_sha256': hashlib.sha256(executable.read_bytes()).hexdigest(),
        'binary_bytes': executable.stat().st_size,
        'platform': platform.system(), 'architecture': platform.machine(),
        'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
        'native_access_run': False,
    }
    (output / 'context.json').write_text(json.dumps(context, indent=2) + '\n')
    print(report.decode(), end='')
    print('Executable bytes:', executable.stat().st_size)
    print('Artifacts:', output)


if __name__ == '__main__':
    main()
