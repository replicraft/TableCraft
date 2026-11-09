"""Independent complete-image oracle for sequential and multi-fragment SDK edits.

Uses only the earlier first-party raw framing helpers. No native production
codec/planner, SDK encoder or SDK-produced AFTER is a construction input.
"""

from pathlib import Path
import hashlib
import importlib.util
import json
import struct


ROOT = Path('.cache/querydef-repeat-save-sdk-20261108')
ORIGINAL = Path('.cache/querydef-existing-save-sdk-20261108/files/multi-page-before.accdb')
PAGE = 4096
OWNER = 4
TARGET = -2147476548
WIDE = TARGET + 2

spec = importlib.util.spec_from_file_location('raw_framing', Path(__file__).with_name('model.py'))
raw = importlib.util.module_from_spec(spec)
spec.loader.exec_module(raw)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def resolve(image, object_id, attribute, ordinal, expression):
    entries = raw.records(image, OWNER)
    matches = []
    for logical, root in entries.items():
        if root['flags'] == 0x8000:
            continue
        assert root['flags'] in (0, 0x4000)
        physical = logical
        if root['flags'] == 0x4000:
            pointer = struct.unpack_from('<I', root['raw'], 0)[0]
            assert root['raw'][4:] == bytes(len(root['raw']) - 4)
            physical = (pointer >> 8, pointer & 255)
            assert entries[physical]['flags'] == 0x8000
        record = entries[physical]
        row = record['raw']
        if struct.unpack_from('<i', row, 2)[0] != object_id or row[6] != attribute:
            continue
        starts, end, _ = raw.framing(row)
        if row[starts[0]:starts[1]] != ordinal.to_bytes(4, 'big'):
            continue
        text = expression.encode('utf-16le')
        descriptor = struct.pack('<I', 0x80000000 | len(text)) + bytes(8) + text
        assert row[starts[3]:end] == descriptor
        matches.append((logical, root, physical, record))
    assert len(matches) == 1
    return matches[0]


def construct(before, edit):
    object_id, attribute, ordinal, expected, replacement = edit
    logical, root, physical, record = resolve(before, object_id, attribute, ordinal, expected)
    row = raw.authored_row(record['raw'], replacement, len(record['raw']))
    after = bytearray(before)
    if len(row) == len(record['raw']):
        base = physical[0] * PAGE
        after[base + record['start']:base + record['end']] = row
    else:
        number, slot = logical
        base = number * PAGE
        page = before[base:base + PAGE]
        count = raw.short(page, 12)
        assert count < 255
        end = raw.short(page, 14 + 2 * (count - 1)) & 0x1fff
        start = end - len(row)
        assert start >= 14 + 2 * (count + 1)
        pointer = struct.pack('<I', (number << 8) | count)
        # Re-forwarding preserves all old hidden targets. Only the live logical
        # root pointer changes; existing pointer-tail bytes remain exact.
        root_bytes = pointer + (root['raw'][4:] if root['flags'] else bytes(len(root['raw']) - 4))
        after[base + root['start']:base + root['end']] = root_bytes
        raw.put_short(after, base + 14 + 2 * slot, root['word'] | 0x4000)
        raw.put_short(after, base + 14 + 2 * count, start | 0x8000)
        raw.put_short(after, base + 12, count + 1)
        raw.put_short(after, base + 2, raw.short(page, 2) - len(row) - 2)
        after[base + start:base + end] = row
    return bytes(after), logical


def main():
    observations = [line.split('\t') for line in (ROOT / 'sdk-observations.tsv').read_text().splitlines()]
    original_hash = digest(ORIGINAL.read_bytes())
    assert original_hash == '33e0e41c2152f4e7e5aadb18e0978b4f224ee225d8042dc08d70528f37cbca05'
    cases = [
        ('repeat-grow', ORIGINAL, [(TARGET, 6, 1, '42', '1000')]),
        ('repeat-shrink', ROOT / 'files/repeat-grow.accdb', [(TARGET, 6, 1, '1000', '7')]),
        ('repeat-grow-again', ROOT / 'files/repeat-shrink.accdb', [(TARGET, 6, 1, '7', '1000000')]),
        ('repeat-noop', ROOT / 'files/repeat-grow-again.accdb', [(TARGET, 6, 1, '1000000', '1000000')]),
        ('two-projections', ORIGINAL, [(WIDE, 6, 96, '95', '1000'), (WIDE, 6, 95, '94', '7000')]),
        ('projection-where', ORIGINAL, [(WIDE, 6, 96, '95', '1000'), (WIDE, 8, 1, '[p].[ID] >= 2', '[p].[ID] >= 1000')]),
    ]
    report = []
    for name, source, edits in cases:
        before = source.read_bytes()
        model = before
        selected = []
        for edit in edits:
            model, logical = construct(model, edit)
            selected.append(logical)
        after_path = ROOT / 'files' / (name + '.accdb')
        after = after_path.read_bytes()
        model_path = ROOT / 'files' / (name + '-literal.accdb')
        if model_path.exists():
            assert model_path.read_bytes() == model
        else:
            model_path.write_bytes(model)
        before_records = raw.records(before, OWNER)
        after_records = raw.records(after, OWNER)
        selected_physical = {resolve(before, *edit[:3], edit[3])[2] for edit in edits}
        permitted_original = set(selected) | selected_physical
        unselected_exact = all(
            record['word'] == after_records[key]['word'] and record['raw'] == after_records[key]['raw']
            for key, record in before_records.items() if key not in permitted_original
        )
        index_pages = [number for number in range(len(before) // PAGE)
                       if before[number * PAGE:number * PAGE + 2] in (b'\x03\x01', b'\x04\x01')]
        before_indexes = {(r[2], r[3]): r[4:] for r in observations
                          if r[0] == 'index' and r[1] == source.name}
        after_indexes = {(r[2], r[3]): r[4:] for r in observations
                         if r[0] == 'index' and r[1] == after_path.name}
        assert before_indexes and after_indexes
        differences = [offset for offset, (old, new) in enumerate(zip(before, after)) if old != new]
        mismatches = [offset for offset, (left, right) in enumerate(zip(model, after)) if left != right]
        live_targets = set()
        for reference, record in after_records.items():
            if record['flags'] == 0x4000:
                pointer = struct.unpack_from('<I', record['raw'], 0)[0]
                target = (pointer >> 8, pointer & 255)
                assert target not in live_targets
                assert after_records[target]['flags'] == 0x8000
                live_targets.add(target)
        retired = [list(reference) for reference, record in after_records.items()
                   if record['flags'] == 0x8000 and reference not in live_targets]
        result = {
            'case': name,
            'before_path': str(source),
            'before_sha256': digest(before),
            'after_path': str(after_path),
            'after_sha256': digest(after),
            'literal_model_path': str(model_path),
            'literal_model_sha256': digest(model),
            'edits': [list(edit) for edit in edits],
            'before_bytes': len(before),
            'after_bytes': len(after),
            'complete_model_matches': model == after,
            'first_model_differences': mismatches[:16],
            'changed_bytes': len(differences),
            'changed_pages': sorted({offset // PAGE for offset in differences}),
            'before_carriers': [raw.carrier(before, OWNER, logical) for logical in selected],
            'after_carriers': [raw.carrier(after, OWNER, logical) for logical in selected],
            'retired_hidden_rows_after': retired,
            'unselected_original_records_unchanged': unselected_exact,
            'sdk_index_memberships_unchanged': before_indexes == after_indexes,
            'index_signature_pages_unchanged': all(
                before[number * PAGE:(number + 1) * PAGE] == after[number * PAGE:(number + 1) * PAGE]
                for number in index_pages
            ),
        }
        report.append(result)
    metadata = {
        'schema': 1,
        'date': '2026-11-08',
        'scope': 'Inert SDK repeated-root and samepage expression updates; V2 not implemented/admitted; native Access not run.',
        'recipe': 'docs/testing/querydef_existing_save/ExistingQueryRepeatFixture.java',
        'model': 'docs/testing/querydef_existing_save/repeat_model.py',
        'dependencies': ['docs/testing/querydef_existing_save/model.py'],
        'original_source_sha256': original_hash,
        'original_source_unchanged': digest(ORIGINAL.read_bytes()) == original_hash,
        'cases': report,
    }
    for key in ['recipe', 'model']:
        metadata[key + '_sha256'] = digest(Path(metadata[key]).read_bytes())
    metadata['raw_framing_sha256'] = digest(Path(__file__).with_name('model.py').read_bytes())
    metadata['sdk_sha256'] = digest(Path('.cache/tablecraft-fixtures/jackcess-5.0.4.jar').read_bytes())
    metadata['sdk_observations_sha256'] = digest((ROOT / 'sdk-observations.tsv').read_bytes())
    metadata['sdk_stderr'] = (ROOT / 'sdk-stderr.log').read_text()
    (ROOT / 'observations.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print([(case['case'], case['complete_model_matches'], case['first_model_differences']) for case in report])
    assert all(case['complete_model_matches'] for case in report)
    assert all(case['unselected_original_records_unchanged'] for case in report)
    assert all(case['sdk_index_memberships_unchanged'] for case in report)
    assert all(case['index_signature_pages_unchanged'] for case in report)


if __name__ == '__main__':
    main()
