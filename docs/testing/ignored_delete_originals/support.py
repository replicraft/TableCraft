"""Repository-local evidence helpers; importing does not capture or mutate artifacts."""

import hashlib
import json
import os
from pathlib import Path
import sys

REPOSITORY_ROOT = next(parent for parent in Path(__file__).resolve().parents
                       if (parent / 'docs/testing/encoded_delete_originals/wire.py').is_file())
sys.path.insert(0, str(REPOSITORY_ROOT / 'docs/testing/encoded_delete_originals'))
import wire

GETTER_CLASS = 'com.healthmarketscience.jackcess.impl.IgnoredDeleteReference'
GETTER_CLASS_FILE = Path('com/healthmarketscience/jackcess/impl/IgnoredDeleteReference.class')
JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
PROCESS_TIMEOUT_SECONDS = 120


def pin(path):
    resolved = path.resolve()
    label = resolved.relative_to(REPOSITORY_ROOT).as_posix()
    data = resolved.read_bytes()
    return {'path': label, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def verify(record):
    assert pin(REPOSITORY_ROOT / record['path']) == record, record['path']


def canonical(value):
    normalized = json.loads(json.dumps(value, ensure_ascii=True))
    return json.dumps(normalized, sort_keys=True, ensure_ascii=True)


def oracle_command(args, path, tables):
    classpath = os.pathsep.join([str(args.jar), str(args.helper_dir)])
    return ['java', '-cp', classpath, GETTER_CLASS, 'oracle', str(path), ','.join(tables)]


def fresh_report(path, report):
    path.resolve().relative_to(REPOSITORY_ROOT)
    with path.open('x') as output:
        output.write(json.dumps(report, indent=2) + '\n')
