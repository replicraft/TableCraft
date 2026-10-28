"""Independent semantic column-map growth on the working SOURCE/request model."""
from pathlib import Path
import hashlib
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_external_long_values import model as ordinary
from ace16_insert_long_value_map_growth import source, requests

PAGE = 4096
BITMAP_SPAN = 32736


class Image(ordinary.Image):
    def __init__(self, state):
        super().__init__(state)
        self.members = {}
        self.bitmap_pages = []
        self.rewrites = []
        for role, value in state['maps'].items():
            if role.startswith(self.owner + ':Column:'):
                self.members[role] = set(value['file_members'])

    def reserve(self, role, kind):
        number = len(self.bytes) // PAGE
        assert number in self.state['certified_future_pages']
        for other, claim in self.state['map_claims'].items():
            cell = source.represented_cell(self.original, claim, number)
            if other == 'GlobalFree':
                assert cell is not None and self.original[cell[0]] & cell[1]
            elif cell is not None:
                assert not self.original[cell[0]] & cell[1], (other, number)
        ordinary.Image.bit(self, 'GlobalFree', number, False)
        page = bytearray(PAGE)
        if kind == 'LVAL':
            page[:2] = b'\x01\x01'
            ordinary.put(page, 2, 4082)
            page[4:8] = b'LVAL'
        else:
            assert kind == 'COLUMN_OWNED_BITMAP'
            page[:4] = b'\x05\x01\x00\x00'
        self.bytes.extend(page)
        self.allocations.append({'page': number, 'role': role, 'kind': kind})
        return number

    def inline(self, role, base):
        value = bytearray(69)
        ordinary.put(value, 1, base, 4)
        for member in sorted(self.members[role]):
            relative = member - base
            assert 0 <= relative < 512
            value[5 + relative // 8] |= 1 << (relative % 8)
        return value

    def rewrite(self, role, value):
        claim = self.state['map_claims'][role]
        assert len(value) == claim['row_end'] - claim['row_start'] == 69
        self.bytes[claim['row_start']:claim['row_end']] = value
        self.rewrites.append({'role': role, 'wire': bytes(value).hex()})
        self.map_offsets.update(range(claim['row_start'], claim['row_end']))

    def bit(self, role, page, present):
        if role not in self.members:
            return ordinary.Image.bit(self, role, page, present)
        members = self.members[role]
        if present:
            assert page not in members
            members.add(page)
        else:
            assert page in members
            members.remove(page)
        claim = self.state['map_claims'][role]
        start, end = claim['row_start'], claim['row_end']
        current = self.bytes[start:end]
        if current[0] == 0:
            base = source.number(current, 1, 4)
            relative = page - base
            if 0 <= relative < 512:
                offset, mask = start + 5 + relative // 8, 1 << (relative % 8)
                self.bytes[offset] = self.bytes[offset] | mask if present else self.bytes[offset] & ~mask
                self.map_offsets.add(offset)
                return
            assert present and members
            base = min(members) // 8 * 8
            if max(members) < base + 512:
                self.rewrite(role, self.inline(role, base))
                return
            assert role.endswith(':Owned'), 'Candidate EOF follows an empty hint set'
            blocks = {member // BITMAP_SPAN for member in members}
            assert blocks == {0}, 'This authentic corpus does not reach a new indirect block'
            bitmap = self.reserve(role, 'COLUMN_OWNED_BITMAP')
            value = bytearray(69)
            value[0] = 1
            ordinary.put(value, 1, bitmap, 4)
            self.rewrite(role, value)
            self.bitmap_pages.append({'page': bitmap, 'role': role, 'block': 0})
            for member in members:
                offset = bitmap * PAGE + 4 + member // 8
                self.bytes[offset] |= 1 << (member % 8)
            return
        assert current[0] == 1 and role.endswith(':Owned')
        block = page // BITMAP_SPAN
        pointer_at = 1 + block * 4
        assert pointer_at + 4 <= len(current)
        bitmap = source.number(current, pointer_at, 4)
        assert bitmap in {value['page'] for value in self.bitmap_pages}
        relative = page % BITMAP_SPAN
        offset, mask = bitmap * PAGE + 4 + relative // 8, 1 << (relative % 8)
        self.bytes[offset] = self.bytes[offset] | mask if present else self.bytes[offset] & ~mask

    def allocate(self, role, lval):
        assert lval, 'Selected DATA remains fitted in this bounded source profile'
        number = self.reserve(role, 'LVAL')
        assert not self.hints[role + ':Hint']
        self.bit(role + ':Owned', number, True)
        self.bit(role + ':Hint', number, True)
        self.hints[role + ':Hint'].append(number)
        return number

    def verify_maps(self):
        result = {}
        for role, members in self.members.items():
            old = self.state['maps'][role]
            description = {
                'table_page': old['map_page'],
                'row_start': old['row_start'] - old['map_page'] * PAGE,
                'row_end': old['row_end'] - old['map_page'] * PAGE,
                'reported_count': len(members), 'file_members': sorted(members),
            }
            value = source.map_wire.decode_map(bytes(self.bytes), description, PAGE, len(self.bytes) // PAGE)
            assert value['file_members'] == sorted(members)
            if role.endswith(':Hint'):
                assert members == set(self.hints[role])
            result[role] = {'kind': value['payload_type'], 'members': sorted(members),
                            'bitmap_pages': value['bitmap_pages']}
        for metadata in self.bitmap_pages:
            assert metadata['page'] not in self.members[metadata['role']]
            hint = metadata['role'].removesuffix(':Owned') + ':Hint'
            assert metadata['page'] not in self.members[hint]
        return result


def derive(state, selected):
    rows = requests.rows(selected)
    assert len(rows) == 1
    image = Image(state)
    compressed = image.table['columns']['Memo']['compressed']
    records = []
    for identity, group, blob, memo in rows:
        descriptors = [image.descriptor(0, blob), image.descriptor(1, ordinary.memo_bytes(memo, compressed))]
        records.append(ordinary.record(identity, group, *descriptors))
    lval_end = len(image.bytes)
    for body in records:
        image.new_rows.append(image.append(image.owner, body, False))
    for physical, column in [(0, 'ID'), (1, 'Group')]:
        image.index(physical, column, rows)
    ordinary.put(image.bytes, image.table['definition'] * PAGE + 16, 7, 4)
    maps = image.verify_maps()
    result = bytes(image.bytes)
    for row in image.table['rows'].values():
        frame, chain = source.long_wire.final_frame(image.original, row)
        assert result[frame['start']:frame['end']] == row['raw']
    for scope in state['scopes'].values():
        for old in scope['live']:
            assert result[old['start']:old['end']] == image.original[old['start']:old['end']]
    for row in state['tables']['Padding']['rows'].values():
        frame, chain = source.long_wire.final_frame(image.original, row)
        assert result[frame['start']:frame['end']] == row['raw']
    for requested, placed in zip(rows, image.new_rows, strict=True):
        body = result[placed['start']:placed['end']]
        for name, expected in [('Blob', requested[2]), ('Memo', requested[3])]:
            value = source.long_wire.long_value(result, body, image.table['columns'][name])['logical']
            if isinstance(value, dict):
                value = bytes.fromhex(value['binary_hex'])
            assert value == expected
    return result, {'sha256': hashlib.sha256(result).hexdigest(), 'bytes': len(result),
                    'allocations': image.allocations, 'lval_phase_end': lval_end,
                    'rows': image.new_rows, 'chunks': image.chunk_rows,
                    'bitmap_pages': image.bitmap_pages, 'maps': maps,
                    'map_rewrites': image.rewrites, 'attempts': image.events}
