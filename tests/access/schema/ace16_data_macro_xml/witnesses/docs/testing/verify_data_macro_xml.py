"""Replay saved table-Extra XML-envelope evidence without launching any process.

The private originals and captured streams must already exist. This verifies
inert envelope facts, not executable macro semantics or qualified absence.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import re
import sys
import xml.etree.ElementTree as ET

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from scripts.compat_artifacts import ArtifactStore

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_RECORD = 'docs/testing/results/data-macro-xml-envelopes-2026-10-29.json'
NAMESPACE = 'http://schemas.microsoft.com/office/accessservices/2009/11/application'
MAX_PAYLOAD_BYTES = 2 * 1024 * 1024
MAX_DEPTH = 128
MAX_ATTRIBUTES = 64
MAX_NODES = 65_536


class Pins:
    def __init__(self, store, captured, retained):
        self.store = store
        self.captured = captured
        self.retained = retained
        self.count = 0
        self.paths = set()

    def check(self, expected):
        selected = expected
        if expected['path'] == self.captured['path']:
            assert expected == self.captured
            selected = self.retained
        actual = self.store.pin(selected['path'])
        assert actual == selected, selected['path']
        self.count += 1
        self.paths.add(selected['path'])
        return self.store.path(selected['path'])


class Cursor:
    def __init__(self, raw):
        self.raw = raw
        self.position = 0

    def take(self, count):
        end = self.position + count
        assert count >= 0 and end <= len(self.raw), 'truncated MR2 frame'
        value = self.raw[self.position:end]
        self.position = end
        return value

    def integer(self, width):
        return int.from_bytes(self.take(width), 'little')

    def remaining(self):
        return len(self.raw) - self.position


def wide_properties(raw):
    assert 0 < len(raw) <= MAX_PAYLOAD_BYTES
    cursor = Cursor(raw)
    assert cursor.take(4) == b'MR2\0'
    names = None
    groups = []
    entries = 0
    while cursor.remaining():
        length = cursor.integer(4)
        kind = cursor.integer(2)
        assert length >= 6
        body = cursor.take(length - 6)
        block = Cursor(body)
        if kind == 128:
            assert names is None and not groups
            names = []
            while block.remaining():
                name = block.take(block.integer(2)).decode('utf-16-le', errors='strict')
                assert name and name not in names
                names.append(name)
                assert len(names) <= 256
            continue
        assert names is not None and kind in (3, 4)
        assert len(groups) < 1024 and kind not in [group['kind'] for group in groups]
        assert block.take(10) == bytes.fromhex('0a000000000000000000')
        properties = []
        while block.remaining():
            length = block.integer(4)
            assert length >= 12
            entry = Cursor(block.take(length - 4))
            flags = entry.integer(1)
            native_type = entry.integer(1)
            index = entry.integer(2)
            size = entry.integer(4)
            value = entry.take(size)
            assert not entry.remaining()
            assert flags == 1 and native_type == 12 and index < len(names)
            name = names[index]
            assert name not in [prop['name'] for prop in properties]
            properties.append({'name': name, 'flags': flags, 'native_type': native_type,
                               'raw': value, 'text': value.decode('utf-16-le', errors='strict')})
            entries += 1
            assert entries <= 4096
        groups.append({'kind': kind, 'body': body, 'properties': properties})
    assert names is not None
    return names, groups


def envelope(raw):
    """Independent safe parsing of the nine original single-root documents."""
    assert len(raw) <= MAX_PAYLOAD_BYTES
    text = raw.decode('utf-16-le', errors='strict')
    assert not re.search(r'<!\s*(?:DOCTYPE|ENTITY)\b', text, re.IGNORECASE)
    processing = list(re.finditer(r'<\?([^\s?]+)', text))
    assert len(processing) == 1 and processing[0].start() == 0
    assert processing[0].group(1) == 'xml'
    parser = ET.XMLPullParser(events=('start', 'end'))
    depth = 0
    nodes = 0
    root = None
    # Original byte encodings, including the UTF-16 declaration, are consumed
    # directly. No path, external resolver or schema loader is provided.
    for start in range(0, len(raw), 4096):
        parser.feed(raw[start:start + 4096])
        for event, element in parser.read_events():
            if event == 'start':
                depth += 1
                nodes += 1
                assert depth <= MAX_DEPTH and nodes <= MAX_NODES
                assert len(element.attrib) <= MAX_ATTRIBUTES
                if root is None:
                    root = element
                else:
                    assert element.tag != '{' + NAMESPACE + '}DataMacro'
            else:
                depth -= 1
                assert depth >= 0
    parser.close()
    assert depth == 0 and root is not None
    assert root.tag == '{' + NAMESPACE + '}DataMacro'
    assert set(root.attrib) in ({'Event'}, {'Name'})
    attribute = next(iter(root.attrib))
    value = root.attrib[attribute]
    assert value
    return NAMESPACE, value if attribute == 'Event' else '', value if attribute == 'Name' else ''


def source_expected(case, golden, pins):
    raw = pins.check(golden['extra']).read_bytes()
    names, groups = wide_properties(raw)
    assert names == golden['expected_properties']['names']
    assert len(groups) == len(golden['expected_properties']['groups'])
    with pins.store.path(golden['source_stream']).open(newline='') as handle:
        matches = [row for row in csv.DictReader(handle, delimiter='\t')
                   if row['Id'] == str(case['origin']['Id'])]
    assert len(matches) == 1
    row = matches[0]
    assert row['ParentId'] == str(case['origin']['ParentId'])
    assert row['Type'] == '1' and json.loads(row['Name']) == case['table']
    assert row['LvExtra'] == '0x' + raw.hex()
    image = pins.store.path(case['image']).read_bytes()
    assert image[20] == 3
    assert case['origin']['Id'] == golden['object_native_id']
    assert case['origin']['ParentId'] == golden['parent_native_id']
    assert case['origin']['Type'] == golden['type_code'] == 1
    origin = case['origin']
    rows = [
        ['Table', case['table']], ['Native ID', str(origin['Id'])],
        ['Native parent ID', str(origin['ParentId'])],
        ['Catalog page', str(origin['page'])], ['Catalog slot', str(origin['slot'])],
        ['LvExtra field', 'Present'], ['Structural coverage', 'Observed wide-MR2 framing'],
        ['Payload bytes', str(len(raw))], ['Property names', str(len(names))],
        ['Value groups', str(len(groups))], ['Data-macro absence', 'Not certified'],
        ['Group kind', 'Group body bytes', 'Property', 'Flags', 'Native type',
         'Value bytes', 'Validated text'],
    ]
    envelopes = []
    for group, recorded in zip(groups, golden['expected_properties']['groups']):
        assert group['kind'] == recorded['kind']
        assert len(group['properties']) == len(recorded['properties'])
        for prop, old in zip(group['properties'], recorded['properties']):
            assert prop['name'] == old['name']
            assert prop['flags'] == old['flags'] and prop['native_type'] == old['native_type']
            assert prop['raw'].hex() == old['raw_hex'] and prop['text'] == old['text']
            assert hashlib.sha256(prop['raw']).hexdigest() == old['sha256']
            namespace, event, name = envelope(prop['raw'])
            envelopes.append([str(group['kind']), prop['name'], namespace, event, name])
            rows.append([str(group['kind']), str(len(group['body'])), prop['name'], '0x01',
                         '12', str(len(prop['raw'])), 'TRUE'])
    rows.append(['Group kind', 'Property', 'XML namespace', 'Event', 'Macro name'])
    rows.extend(envelopes)
    rows.extend([
        ['XML documents inspected', str(len(envelopes))],
        ['XML envelope definitions', str(len(envelopes))],
        ['XML envelope coverage', 'Envelope metadata only; runtime semantics not validated'
         if envelopes else 'No XML documents inspected; absence not certified'],
    ])
    return {'rows': rows, 'documents': len(envelopes), 'definitions': len(envelopes),
            'envelopes': envelopes, 'names_only': not envelopes}


def verify(record_path):
    store = ArtifactStore(ROOT)
    record = store.load(record_path)
    comparison = record['original_comparison']
    bound_path = store.path(comparison['manifest']['path'])
    bound = store.load(comparison['manifest']['path'])
    captured = comparison['captured_consumer']
    retained = comparison['consumer']
    assert captured == bound['cli']
    captured_path = Path(captured['path'])
    assert not captured_path.is_absolute() and '..' not in captured_path.parts
    assert captured_path.as_posix() == captured['path']
    captured_argv = str(ROOT / captured_path)
    assert (retained['bytes'], retained['sha256']) == (captured['bytes'], captured['sha256'])
    pins = Pins(store, captured, retained)
    pins.check(retained)

    def record_pins(value):
        if isinstance(value, dict):
            if all(key in value for key in ('path', 'bytes', 'sha256')):
                pins.check(value)
            for child in value.values():
                record_pins(child)
        elif isinstance(value, list):
            for child in value:
                record_pins(child)

    record_pins(record)
    for item in record['source_files']:
        pins.check(item)
    for key in ['manifest', 'plan', 'freeze', 'report']:
        pins.check(comparison[key])
    assert bound_path == store.path(bound['plan']['path']).parent / 'bound.json'
    for item in bound['inputs']:
        pins.check(item)
    for item in store.load(comparison['freeze']['path'])['inputs']:
        pins.check(item)
    plan = store.load(comparison['plan']['path'])
    assert bound['plan'] == comparison['plan']
    for item in plan['inputs']:
        pins.check(item)
    old_plan_pin = next(item for item in plan['inputs'] if item['path'].endswith('original-consumer-next-20261029/plan.json'))
    goldens_pin = next(item for item in plan['inputs'] if item['path'].endswith('original-goldens-20261029/goldens.json'))
    original = store.load(old_plan_pin['path'])
    goldens = store.load(goldens_pin['path'])
    assert len(original['calls']) == len(plan['calls']) == len(goldens['items']) == 7
    expected = []
    for call, old in zip(plan['calls'], original['calls']):
        assert call['id'] == old['id'] and call['image'] == old['image'] and call['table'] == old['table']
        facts = source_expected(old, goldens['items'][old['golden_index']], pins)
        assert facts == call['expected']
        assert call['arguments'] == [
            'inspect', call['image'], '--table', call['table'], '--extra', '--macro-xml',
            '--format', 'tsv', '--native-bytes', '67108864', '--native-steps', '10000000',
        ]
        expected.append(facts)
    # All source-derived expectations precede reads of any newly captured output.
    report = store.load(comparison['report']['path'])
    assert report['completed'] and not report['after_pin_errors']
    assert report['manifest'] == comparison['manifest']
    assert len(report['commands']) == len(report['comparisons']) == 7
    for call, facts, command, observed in zip(plan['calls'], expected, report['commands'], report['comparisons']):
        assert command['status'] == 0 and command['stopped_for'] is None and not command['errors']
        # This records the historical executable location. The retained copy is
        # verified above; mutable build output is never resolved or reopened.
        assert command['argv'] == [captured_argv, *call['arguments']]
        assert command['timeout_seconds'] == call['timeout_seconds'] == 30
        assert command['stream_caps'] == [call['stdout_cap'], call['stderr_cap']] == [1_048_576, 65_536]
        assert command['observed_bytes'] == [command['stdout']['bytes'], command['stderr']['bytes']]
        assert command['stderr']['bytes'] == 0
        pins.check(command['stderr'])
        raw = pins.check(command['stdout']).read_text()
        actual = list(csv.reader(io.StringIO(raw), delimiter='\t'))
        assert actual == facts['rows']
        assert observed == {'id': call['id'], 'exact_records': len(actual),
                            'documents': facts['documents'], 'definitions': facts['definitions'],
                            'names_only': facts['names_only']}
    for item in [retained, *record['source_files'], *bound['inputs'], *plan['inputs']]:
        pins.check(item)
    assert sum(len(facts['rows']) for facts in expected) == 130
    assert sum(facts['documents'] for facts in expected) == 9
    assert sum(facts['names_only'] for facts in expected) == 2
    for name, count in [('commands', 7), ('exact_records', 130), ('documents', 9),
                        ('definitions', 9), ('names_only', 2)]:
        assert comparison[name] == count
    assert comparison['all_input_pins_unchanged'] is True
    return {'verified': True, 'calls': 7, 'exact_records': 130, 'documents': 9,
            'definitions': 9, 'names_only': 2, 'pins': pins.count,
            'paths': len(pins.paths), 'children': 0, 'absence_qualified': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', default=DEFAULT_RECORD)
    args = parser.parse_args()
    try:
        result = verify(args.record)
    except (OSError, ValueError, TypeError, KeyError, AssertionError, ET.ParseError, StopIteration) as error:
        print(json.dumps({'verified': False, 'error': type(error).__name__, 'message': str(error)}))
        return 2
    print(json.dumps(result))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
