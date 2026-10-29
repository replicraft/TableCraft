"""SOURCE/request-only GLOBAL bootstrap; no producer or Java AFTER inputs."""
from pathlib import Path
import hashlib
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_external_long_values import model as ordinary
from ace16_insert_long_value_map_growth import model as columns
from ace16_insert_global_bootstrap import source, requests

PAGE = 4096
INLINE_END = 512
FILLED_BITMAP_END = 4092
PHYSICAL_BITMAP_BITS = 32736


class Image(columns.Image):
    def __init__(self, state):
        super().__init__(state)
        assert len(self.original) == INLINE_END * PAGE
        assert state['bootstrap']['global_reference'] == 256
        self.global_bitmap = None
        self.retired = sorted(state['maps']['GlobalFree']['file_members'])
        self.data_owned = set(state['maps'][self.owner + ':Owned']['file_members'])
        self.data_hints = set(state['maps'][self.owner + ':Hint']['file_members'])
        allocated = set(map(int, state['census']['protected_pages']))
        allocated |= set(map(int, state['census']['owned_claims']))
        allocated |= set(map(int, state['census']['live_claims']))
        assert allocated.isdisjoint(self.retired)
        for page in allocated:
            assert page < INLINE_END
            offset, mask = source.map_cell(self.original, state['map_claims']['GlobalFree'], page)
            assert not self.original[offset] & mask

    def fresh(self, number):
        assert number in self.state['certified_future_pages']
        for role, claim in self.state['map_claims'].items():
            cell = source.represented_cell(self.original, claim, number)
            if role == 'GlobalFree':
                assert cell is None and self.state['bootstrap']['implicit_out_of_range_FREE']
            elif cell is not None:
                assert not self.original[cell[0]] & cell[1], (role, number)

    def push(self, role, kind):
        number = len(self.bytes) // PAGE
        self.fresh(number)
        page = bytearray(PAGE)
        if kind in ('LVAL', 'DATA'):
            page[:2] = b'\x01\x01'
            ordinary.put(page, 2, 4082)
            page[4:8] = b'LVAL' if kind == 'LVAL' else self.table['definition'].to_bytes(4, 'little')
        else:
            assert kind in ('GLOBAL_BITMAP', 'COLUMN_OWNED_BITMAP')
            page[:4] = b'\x05\x01\x00\x00'
        self.bytes.extend(page)
        self.allocations.append({'page': number, 'role': role, 'kind': kind})
        return number

    def used(self, number):
        assert self.global_bitmap is not None and number < PHYSICAL_BITMAP_BITS
        offset = self.global_bitmap * PAGE + 4 + number // 8
        self.bytes[offset] &= ~(1 << (number % 8))

    def bootstrap(self, frontier):
        assert self.global_bitmap is None and frontier == 512
        metadata = self.push('GlobalFree', 'GLOBAL_BITMAP')
        assert metadata == 513
        self.global_bitmap = metadata
        base = metadata * PAGE
        self.bytes[base + 4:base + FILLED_BITMAP_END] = bytes([255]) * (FILLED_BITMAP_END - 4)
        # Root-approved frontier retirement includes old FREE holes. Their
        # physical page bytes are protected separately below.
        for number in range(frontier + 1):
            self.used(number)
        self.used(metadata)
        row = bytearray(69)
        row[0] = 1
        ordinary.put(row, 1, metadata, 4)
        self.rewrite('GlobalFree', row)

    def reserve(self, role, kind):
        number = self.push(role, kind)
        if self.global_bitmap is None:
            self.bootstrap(number)
        else:
            self.used(number)
        return number

    def bit(self, role, page, present):
        if role == 'GlobalFree':
            assert not present
            self.used(page)
            return
        if role == self.owner + ':Owned':
            assert present and page not in self.data_owned
            self.data_owned.add(page)
            return ordinary.Image.bit(self, role, page, present)
        if role != self.owner + ':Hint':
            return super().bit(role, page, present)
        if present:
            assert page not in self.data_hints
            self.data_hints.add(page)
        else:
            assert page in self.data_hints
            self.data_hints.remove(page)
        claim = self.state['map_claims'][role]
        start, end = claim['row_start'], claim['row_end']
        current = self.bytes[start:end]
        assert len(current) == 69 and current[0] == 0
        base = source.number(current, 1, 4)
        relative = page - base
        if 0 <= relative < 512:
            offset, mask = start + 5 + relative // 8, 1 << (relative % 8)
            self.bytes[offset] = self.bytes[offset] | mask if present else self.bytes[offset] & ~mask
            self.map_offsets.add(offset)
            return
        assert present and self.data_hints
        base = min(self.data_hints) // 8 * 8
        row = bytearray(69)
        ordinary.put(row, 1, base, 4)
        for member in self.data_hints:
            relative = member - base
            assert 0 <= relative < 512
            row[5 + relative // 8] |= 1 << (relative % 8)
        self.rewrite(role, row)

    def allocate(self, role, lval):
        if lval:
            return super().allocate(role, lval)
        assert role == self.owner and not self.hints[role + ':Hint']
        number = self.reserve(role, 'DATA')
        self.bit(role + ':Owned', number, True)
        self.bit(role + ':Hint', number, True)
        self.hints[role + ':Hint'].append(number)
        return number

    def decoded_map(self, role, members, total=None):
        old = self.state['maps'][role]
        description = {'table_page': old['map_page'],
            'row_start': old['row_start'] - old['map_page'] * PAGE,
            'row_end': old['row_end'] - old['map_page'] * PAGE,
            'reported_count': len(members) if total is None else total,
            'file_members': sorted(members)}
        return source.map_wire.decode_map(bytes(self.bytes), description, PAGE, len(self.bytes) // PAGE)

    def verify_maps(self):
        result = super().verify_maps()
        for suffix, members in [(':Owned', self.data_owned), (':Hint', self.data_hints)]:
            role = self.owner + suffix
            value = self.decoded_map(role, members)
            result[role] = {'kind': value['payload_type'], 'members': sorted(members),
                            'bitmap_pages': value['bitmap_pages']}
        assert self.data_hints == set(self.hints[self.owner + ':Hint'])
        expected_free = (FILLED_BITMAP_END - 4) * 8 - len(self.bytes) // PAGE
        value = self.decoded_map('GlobalFree', set(), expected_free)
        assert value['payload_type'] == 1 and value['bitmap_pages'] == [513]
        base = 513 * PAGE
        assert self.bytes[base + PAGE - 4:base + PAGE] == bytes(4)
        for page in range(len(self.bytes) // PAGE):
            offset, mask = base + 4 + page // 8, 1 << (page % 8)
            assert not self.bytes[offset] & mask
        assert 513 not in self.data_owned and 513 not in self.data_hints
        for members in self.members.values():
            assert 513 not in members
        result['GlobalFree'] = {'kind': 1, 'bitmap_pages': [513],
                                'complete_set_bit_count': value['complete_set_bit_count'],
                                'retired_old_FREE': self.retired, 'physical_span_bits': PHYSICAL_BITMAP_BITS,
                                'initial_FREE_fill_end_exclusive': FILLED_BITMAP_END,
                                'last4bytes': bytes(self.bytes[base + PAGE - 4:base + PAGE]).hex()}
        return result


def derive(state, selected):
    rows = requests.rows(selected)
    image = Image(state)
    records = []
    for identity, group, blob, memo in rows:
        descriptors = [image.descriptor(0, blob),
            image.descriptor(1, ordinary.memo_bytes(memo, image.table['columns']['Memo']['compressed']))]
        records.append(ordinary.record(identity, group, *descriptors))
    lval_end = len(image.bytes)
    for body in records:
        image.new_rows.append(image.append(image.owner, body, False))
    for physical, column in [(0, 'ID'), (1, 'Group')]:
        image.index(physical, column, rows)
    ordinary.put(image.bytes, image.table['definition'] * PAGE + 16, 6 + len(rows), 4)
    maps = image.verify_maps()
    result = bytes(image.bytes)
    global_claim = state['map_claims']['GlobalFree']
    assert result[PAGE:global_claim['row_start']] == image.original[PAGE:global_claim['row_start']]
    assert result[global_claim['row_end']:2 * PAGE] == image.original[global_claim['row_end']:2 * PAGE]
    for table in ('Items', 'Padding', 'Sentinel'):
        for old in state['tables'][table]['rows'].values():
            frame, chain = source.long_wire.final_frame(image.original, old)
            assert result[frame['start']:frame['end']] == old['raw']
    for scope in state['scopes'].values():
        for old in scope['live']:
            assert result[old['start']:old['end']] == image.original[old['start']:old['end']]
    for page in image.retired:
        assert result[page * PAGE:(page + 1) * PAGE] == image.original[page * PAGE:(page + 1) * PAGE]
    for requested, placed in zip(rows, image.new_rows, strict=True):
        body = result[placed['start']:placed['end']]
        assert int.from_bytes(body[2:6], 'little', signed=True) == requested[0]
        assert int.from_bytes(body[6:10], 'little', signed=True) == requested[1]
        for name, wanted in [('Blob', requested[2]), ('Memo', requested[3])]:
            actual = source.long_wire.long_value(result, body, image.table['columns'][name])['logical']
            if isinstance(actual, dict):
                actual = bytes.fromhex(actual['binary_hex'])
            assert actual == wanted
    for physical, column in [(0, 'ID'), (1, 'Group')]:
        tree = state['trees'][physical]
        root = tree['leaves'][0]
        _, leaves, entries = source.index_wire.tree(result, root, image.table['definition'])
        assert leaves == [root]
        expected = list(tree['entries'])
        expected.extend(source.index_wire.key(row[0 if column == 'ID' else 1], placed['reference'])
                        for row, placed in zip(rows, image.new_rows, strict=True))
        assert entries == sorted(expected)
    if selected == 'inline':
        assert len(image.new_rows) == 32 and len(image.chunk_rows) == 0
        assert all(row['reference'] >> 8 == state['bootstrap']['existing_DATA'] for row in image.new_rows[:21])
        assert [row['reference'] for row in image.new_rows[21:]] == [512 << 8 | slot for slot in range(11)]
    return result, {'sha256': hashlib.sha256(result).hexdigest(), 'bytes': len(result),
                    'allocations': image.allocations, 'lval_phase_end': lval_end,
                    'rows': image.new_rows, 'chunks': image.chunk_rows,
                    'maps': maps, 'map_rewrites': image.rewrites, 'attempts': image.events,
                    'retired_old_FREE': image.retired, 'old_hole_page_bytes_preserved': True}
