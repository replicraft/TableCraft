"""Export independent raw Number-growth images, then observe read-only SDK getters."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

from engineering_paths import repository_root
from saved_field_rename.run import SDK_SHA256, execute

HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--java-bin', type=Path, required=True)
    args = parser.parse_args()
    repository = repository_root(__file__)
    output = args.output_dir.resolve()
    output.relative_to(repository / '.cache')
    if digest(args.jar) != SDK_SHA256:
        parser.error('expected the reviewed Jackcess 5.0.4 artifact')
    output.mkdir(parents=True, exist_ok=False)

    built = execute([
        'cargo', 'build', '--offline', '--locked', '-p', 'tablecraft-testkit',
        '--no-default-features', '--message-format=json',
    ], output, 'build-testkit')
    if built.returncode:
        raise RuntimeError('testkit build failed; see retained evidence')
    artifacts = [
        Path(name)
        for line in built.stdout.splitlines()
        if line.startswith('{')
        for artifact in [json.loads(line)]
        if artifact.get('reason') == 'compiler-artifact'
        and artifact['target']['name'] == 'tablecraft_testkit'
        for name in artifact['filenames']
        if name.endswith('.rlib')
    ]
    if len(artifacts) != 1:
        raise RuntimeError('expected one exact Cargo-produced testkit library')
    executable = output / ('export.exe' if os.name == 'nt' else 'export')
    compiled = execute([
        'rustc', '--edition=2024', '-O', str(HERE / 'export.rs'),
        '--extern', 'tablecraft_testkit=' + str(artifacts[0]),
        '-L', 'dependency=' + str(artifacts[0].parent), '-o', str(executable),
        '-L', 'dependency=' + str(artifacts[0].parent / 'deps'),
    ], output, 'compile-export')
    if compiled.returncode:
        raise RuntimeError('raw exporter compilation failed; see retained evidence')
    exports = output / 'exports'
    exports.mkdir()
    try:
        exported = subprocess.run(
            [str(executable), str(exports)], capture_output=True, text=True, timeout=60,
        )
    except subprocess.TimeoutExpired as error:
        (output / 'export.stderr').write_text('Exporter exceeded 60 seconds before completion.\n')
        raise RuntimeError('raw export timed out; evidence retained') from error
    (output / 'export.stdout').write_text(exported.stdout)
    (output / 'export.stderr').write_text(exported.stderr)
    if exported.returncode:
        raise RuntimeError('raw export failed; see retained evidence')

    files = sorted(exports.glob('*.accdb'))
    if len(files) != 12:
        raise RuntimeError('expected twelve independently authored fixture images')
    inputs = {path.name: digest(path) for path in files}
    pins = {path.name: digest(path) for path in [
        HERE / 'export.rs', HERE / 'ReadMixed.java', HERE / 'observe.py',
    ]}
    (output / 'inputs-before-observation.json').write_text(json.dumps({
        'sdk_version': '5.0.4', 'sdk_sha256': SDK_SHA256,
        'sources': pins, 'images': inputs,
        'mode': 'Read-only; expressions disabled; no linked sources or SDK mutation',
    }, indent=2) + '\n')

    classes = output / 'classes'
    classes.mkdir()
    compiled = execute([
        str(args.java_bin / 'javac'), '-cp', str(args.jar), '-d', str(classes),
        str(HERE / 'ReadMixed.java'),
    ], output, 'compile-sdk-observer')
    if compiled.returncode:
        raise RuntimeError('SDK observer compilation failed; see retained evidence')
    observed = execute([
        str(args.java_bin / 'java'), '-Djava.awt.headless=true', '-Xmx256m',
        '-cp', str(classes) + os.pathsep + str(args.jar), 'ReadMixed',
        *map(str, files),
    ], output, 'read')
    unchanged = all(digest(path) == inputs[path.name] for path in files)
    receipt = {
        'sdk_version': '5.0.4', 'sdk_sha256': SDK_SHA256,
        'images': inputs, 'cases': len(files), 'rows_per_case': 3,
        'indexed_rows_per_case': 3, 'sdk_exit': observed.returncode,
        'all_images_unchanged': unchanged,
        'scope': 'Independent SDK reads only; not SDK writes or Microsoft Access saves',
    }
    (output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    if observed.returncode or not unchanged:
        raise RuntimeError('SDK read qualification failed; evidence retained')
    print('12 cases: sequential/indexed values match; all original bytes unchanged')


if __name__ == '__main__':
    main()
