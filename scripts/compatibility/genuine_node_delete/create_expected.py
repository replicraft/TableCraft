"""Explicit pure-source reproduction; never imports a native writer or JVM."""
import argparse
import hashlib
import json
from pathlib import Path

import model

HERE = Path(__file__).resolve().parent


def encoded(value):
    return (json.dumps(value, indent=2) + '\n').encode()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--originals', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not __debug__ or args.output.exists() or args.output.is_symlink():
        raise ValueError('Enabled source assertions and fresh output required')
    requests = json.loads((HERE / 'requests.json').read_text())
    pending = []
    for profile in requests['profiles']:
        header = profile['header']
        before = (args.originals / f'h{header}-node-index-original.accdb').read_bytes()
        after, proof = model.construct(before, profile)
        prefix = f'h{header}-retained-node-delete'
        pending.extend([(prefix + '-before.accdb', before),
                        (prefix + '-after.accdb', after),
                        (prefix + '.proof.json', encoded(proof))])
    args.output.mkdir(parents=True, exist_ok=False)
    files = []
    for name, raw in pending:
        with (args.output / name).open('xb') as handle:
            handle.write(raw)
        files.append({'file': name, 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()})
    with (args.output / 'manifest.json').open('xb') as handle:
        handle.write(encoded({'files': files, 'native_producer_calls': 0, 'child_calls': 0}))


if __name__ == '__main__':
    main()
