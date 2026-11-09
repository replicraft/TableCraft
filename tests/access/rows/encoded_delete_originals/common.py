"""Local artifact/provenance utilities; importing performs no artifact reads."""

import hashlib
import os
from pathlib import Path
import json

REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
GETTER_CLASS = 'com.healthmarketscience.jackcess.impl.EncodedDeleteFixture'
GETTER_CLASS_FILE = Path('com/healthmarketscience/jackcess/impl/EncodedDeleteFixture.class')
PROFILES = ['V2016', 'V2019']
POSITIVE = ['TextLegacyLeaf', 'TextModernLeaf', 'BinaryLeaf',
            'TextLegacyNode', 'TextModernNode', 'BinaryNode']
NEGATIVE = ['TextUniqueOverflow', 'MemoNegative', 'TextCompositeNegative',
            'BinaryCompositeNegative', 'TextIgnoredNegative', 'BinaryIgnoredNegative']
STATES = ['before', 'delete-tie', 'delete-null', 'delete-all']


def pin(path):
    resolved = path.resolve()
    label = resolved.relative_to(REPOSITORY_ROOT).as_posix()
    data = resolved.read_bytes()
    return {'path': label, 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


def write_report(path, report):
    with path.open('x') as output:
        output.write(json.dumps(report, indent=2) + '\n')


def add_getter_arguments(parser):
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--helper-dir', type=Path, required=True)
    parser.add_argument('--helper-source', type=Path, required=True)


def check_getter(args):
    assert pin(args.jar)['sha256'] == JAR_SHA256
    assert (args.helper_dir / GETTER_CLASS_FILE).is_file()
    assert args.helper_source.is_file()


def oracle_command(args, database):
    return ['java', '-cp', os.pathsep.join([str(args.jar), str(args.helper_dir)]),
            GETTER_CLASS, 'oracle', str(database)]
