"""Independent mixed-map source adapter and exact ordinary/LVAL expectations."""
import hashlib
import importlib.util
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from compressed_memo_observations import capture as decode

RAW_MODEL = Path(__file__).with_name('raw_placement.py')
RAW_MODEL_SHA256 = 'b35b1aa9febd62a10e752a4826b415c1984bbd027e1e44581ba520bb5a5aa3ca'
PAGE = 4096
SLOT_OFFSET = 14
SLOT_BYTES = 2
COUNT_OFFSET = 12
FOOTER_BYTES = 9
SELECTED = 'IndirectLongValues'


def original_model():
    data = RAW_MODEL.read_bytes()
    assert hashlib.sha256(data).hexdigest() == RAW_MODEL_SHA256
    spec = importlib.util.spec_from_file_location('independent_raw_lval_model', RAW_MODEL)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def replacement(description):
    if description['kind'] == 'null':
        return None
    assert description['kind'] == 'text' and description['unit'] == 256
    return chr(description['unit']).encode('utf-16le') * description['units']


def state(snapshot):
    owner = snapshot['tables'][SELECTED]['definition']
    roles = {
        'GlobalFree': 'GlobalFree',
        'TableOwned': f'Table:{owner}:Owned',
        'TableHint': f'Table:{owner}:Hint',
        'MemoOwned': f'Table:{owner}:Column:1:Owned',
        'MemoHint': f'Table:{owner}:Column:1:Hint',
        'OLEOwned': f'Table:{owner}:Column:2:Owned',
        'OLEHint': f'Table:{owner}:Column:2:Hint',
    }
    maps = {}
    for name, role in roles.items():
        maps[name] = {key: value for key, value in snapshot['maps'][role].items()
                      if key not in ('bit_locations', 'raw_row_hex', 'all_members')}
    fields = {}
    for name, field in snapshot['long_values']['1']['fields'].items():
        fields[name] = {
            'present': field['present'], 'kind': field['kind'],
            'encoded_bytes': field['encoded_bytes'],
            'encoded_sha256': field['encoded_sha256'],
            'wire_hex': field['descriptor_hex'],
            'chunks': [{key: value for key, value in chunk.items() if key != 'body_hex'}
                       for chunk in field['chunks']],
        }
    path = [{key: value for key, value in frame.items() if key != 'body_hex'}
            for frame in snapshot['long_values']['1']['path']]
    return {'maps': maps, 'fields': fields, 'root_path': path}


def columns(snapshot):
    result = {}
    for field in snapshot['declarations'][SELECTED]['columns']:
        if field['name'] in ('Memo', 'OLE'):
            raw = bytes.fromhex(field['raw_hex'])
            result[field['name']] = {'number': field['number'], 'native_type': field['native'],
                                     'variable': int.from_bytes(raw[7:9], 'little')}
    assert result == {'Memo': {'number': 1, 'native_type': 12, 'variable': 0},
                      'OLE': {'number': 2, 'native_type': 11, 'variable': 1}}
    return result


def preserved_old_rows(before, after, snapshot):
    model = original_model()
    owner = snapshot['tables'][SELECTED]['definition']
    comparisons = 0
    for role in (f'Table:{owner}:Column:1:Owned', f'Table:{owner}:Column:2:Owned'):
        for number in snapshot['maps'][role]['file_members']:
            count = model.number(before, number * PAGE + COUNT_OFFSET)
            for slot in range(count):
                word, start, end = model.frame(before, number, slot)
                assert after[start:end] == before[start:end]
                at = number * PAGE + SLOT_OFFSET + SLOT_BYTES * slot
                assert after[at:at + SLOT_BYTES] == before[at:at + SLOT_BYTES]
                assert word & 0x6000 == 0
                comparisons += 1
    return comparisons


def expected_image(case, before, snapshot):
    assert len(before) == case['source']['bytes']
    assert hashlib.sha256(before).hexdigest() == case['source']['sha256']
    owner = snapshot['tables'][SELECTED]['definition']
    original = state(snapshot)
    frame = original['root_path'][-1]
    assert frame['body_bytes'] == 39
    assert int.from_bytes(before[frame['start']:frame['start'] + 2], 'little') == 3
    assert int.from_bytes(before[frame['end'] - 3:frame['end'] - 1], 'little') == 2
    assert len(before[frame['start']:frame['end']]) - FOOTER_BYTES == 30
    model = original_model()
    before_payload = model.payload(before, original['fields']['Memo'])
    assert before_payload == b'\x00\x01' * 8192
    assert original['maps']['MemoOwned']['payload_type'] == 1
    assert original['maps']['MemoHint']['payload_type'] == 0
    expected, facts = model.expected(before, original, 'Memo', replacement(case['replacement']), owner)
    for row_id, location in snapshot['rows'][SELECTED].items():
        final, path = decode.final_frame(expected, location)
        body = expected[final['start']:final['end']]
        observed = {name: decode.long_value(expected, body, column)
                    for name, column in columns(snapshot).items()}
        wanted = replacement(case['replacement']) if row_id == '1' else bytes.fromhex(snapshot['long_values'][row_id]['fields']['Memo']['encoded_hex'])
        actual = None if observed['Memo']['kind'] == 'NULL' else bytes.fromhex(observed['Memo']['encoded_hex'])
        assert actual == wanted
        assert observed['OLE']['encoded_hex'] == snapshot['long_values'][row_id]['fields']['OLE']['encoded_hex']
        if row_id != '1':
            old = snapshot['long_values'][row_id]['path'][-1]
            assert expected[old['start']:old['end']] == before[old['start']:old['end']]
        assert path[0]['page'] == location['page'] and path[0]['slot'] == location['slot']
    comparisons = preserved_old_rows(before, expected, snapshot)
    index_pages = set()
    for role, description in snapshot['maps'].items():
        if ':Index:' in role:
            index_pages.update(description['file_members'])
    for number in index_pages:
        assert expected[number * PAGE:(number + 1) * PAGE] == before[number * PAGE:(number + 1) * PAGE]
    unchanged_rows = 0
    for table, rows in snapshot['rows'].items():
        for identity, row in rows.items():
            if table == SELECTED and identity == '1':
                continue
            old_word, start, end = model.frame(before, row['page'], row['slot'])
            assert expected[start:end] == before[start:end]
            at = row['page'] * PAGE + SLOT_OFFSET + row['slot'] * SLOT_BYTES
            assert expected[at:at + SLOT_BYTES] == before[at:at + SLOT_BYTES]
            unchanged_rows += 1
    for role in ('MemoOwned', 'TableOwned', 'GlobalFree'):
        description = original['maps'][role]
        if description['payload_type'] == 1:
            start, end = description['row_start'], description['row_end']
            assert expected[start:end] == before[start:end]
            for number in description['bitmap_pages']:
                assert expected[number * PAGE:number * PAGE + 4] == before[number * PAGE:number * PAGE + 4]
    facts.update(original_index_pages_preserved=len(index_pages),
                 unchanged_original_row_body_and_slot_comparisons=unchanged_rows,
                 original_lval_row_body_and_slot_comparisons=comparisons,
                 both_logical_rows_decoded=True, sibling_ole_payloads_preserved=True,
                 original_index_roots_preserved=True)
    return expected, facts
