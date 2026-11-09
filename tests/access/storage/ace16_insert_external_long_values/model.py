"""SOURCE plus immutable requests only; no Java AFTER or native candidate inputs."""
import hashlib

from ace16_insert_external_long_values import requests, source

PAGE = 4096


def put(raw, offset, value, width=2):
    raw[offset:offset + width] = value.to_bytes(width, 'little')


def memo_bytes(value, compressed):
    if value is None:
        return None
    raw = value.encode('utf-16le')
    units = [int.from_bytes(raw[at:at + 2], 'little') for at in range(0, len(raw), 2)]
    if compressed and 3 <= len(units) <= 1024 and all(1 <= unit <= 255 for unit in units):
        raw = b'\xff\xfe' + bytes(units)
    assert source.long_wire.decode_stream(raw) == value
    return raw


def inline(value):
    if value is None:
        return b''
    return (0x80000000 | len(value)).to_bytes(4, 'little') + bytes(8) + value


def record(identity, group, blob, memo):
    body = b'\x04\x00' + identity.to_bytes(4, 'little', signed=True)
    body += (0 if group is None else group).to_bytes(4, 'little', signed=True)
    body += blob + memo
    end = len(body)
    footer = end.to_bytes(2, 'little') + (10 + len(blob)).to_bytes(2, 'little')
    footer += b'\x0a\x00\x02\x00'
    mask = 1 | (group is not None) * 2 | bool(blob) * 4 | bool(memo) * 8
    return body + footer + bytes([mask])


def geometry(raw, page):
    base = page * PAGE
    count = source.number(raw, base + 12)
    assert count <= 255
    end = PAGE if not count else source.number(raw, base + 14 + (count - 1) * 2) & 0x1fff
    assert 14 + count * 2 <= end <= PAGE
    assert source.number(raw, base + 2) == end - 14 - count * 2
    return count, end


class Image:
    def __init__(self, state):
        self.state = state
        self.original = state['raw']
        self.bytes = bytearray(self.original)
        self.table = state['tables']['Items']
        self.owner = 'Table:' + str(self.table['definition'])
        self.hints = {
            role: list(value['file_members']) for role, value in state['maps'].items()
            if role == self.owner + ':Hint' or role.startswith(self.owner + ':Column:') and role.endswith(':Hint')
        }
        self.events = []
        self.allocations = []
        self.new_rows = []
        self.chunk_rows = []
        self.map_offsets = set()

    def bit(self, role, page, present):
        offset, mask = source.map_cell(self.original, self.state['map_claims'][role], page)
        self.bytes[offset] = self.bytes[offset] | mask if present else self.bytes[offset] & ~mask
        self.map_offsets.add(offset)

    def allocate(self, role, lval):
        number = len(self.bytes) // PAGE
        assert number in self.state['certified_future_pages']
        for other, claim in self.state['map_claims'].items():
            offset, mask = source.map_cell(self.original, claim, number)
            assert bool(self.original[offset] & mask) == (other == 'GlobalFree')
        self.bit(role + ':Owned', number, True)
        self.bit(role + ':Hint', number, True)
        self.bit('GlobalFree', number, False)
        self.hints[role + ':Hint'].append(number)
        body = bytearray(PAGE)
        body[:2] = b'\x01\x01'
        put(body, 2, 4082)
        body[4:8] = b'LVAL' if lval else self.table['definition'].to_bytes(4, 'little')
        self.bytes.extend(body)
        self.allocations.append({'page': number, 'role': role, 'kind': 'LVAL' if lval else 'DATA'})
        return number

    def destination(self, role, length, lval):
        hints = self.hints[role + ':Hint']
        attempts = 0
        while hints:
            page = hints[-1]
            count, end = geometry(self.bytes, page)
            fitted = count < 255 and length + 2 <= end - 14 - count * 2
            self.events.append({'phase': 'LVAL' if lval else 'DATA', 'page': page,
                                'bytes': length, 'fitted': fitted})
            attempts += 1
            if fitted:
                return page
            hints.pop()
            self.bit(role + ':Hint', page, False)
            if not lval:
                assert attempts == 1
                break
        return self.allocate(role, lval)

    def append(self, role, payload, lval):
        assert len(payload) + 2 <= 4082
        page = self.destination(role, len(payload), lval)
        count, end = geometry(self.bytes, page)
        base = page * PAGE
        start = end - len(payload)
        assert start >= 14 + (count + 1) * 2
        self.bytes[base + start:base + end] = payload
        put(self.bytes, base + 14 + count * 2, start)
        put(self.bytes, base + 12, count + 1)
        put(self.bytes, base + 2, start - 14 - (count + 1) * 2)
        return {'reference': page << 8 | count, 'start': base + start,
                'end': base + end, 'role': role}

    def descriptor(self, ordinal, payload):
        if payload is None or len(payload) <= 64:
            return inline(payload)
        role = self.owner + ':Column:' + str(ordinal + 2)
        chained = len(payload) > 4076
        pieces = [payload] if not chained else [payload[at:at + 4072] for at in range(0, len(payload), 4072)]
        placed = []
        for piece in pieces:
            placed.append(self.append(role, bytes(4) + piece if chained else piece, True))
        if chained:
            for position, part in enumerate(placed):
                following = placed[position + 1]['reference'] if position + 1 < len(placed) else 0
                put(self.bytes, part['start'], following, 4)
        self.chunk_rows.extend(placed)
        tagged = len(payload) | (0 if chained else 0x40000000)
        return tagged.to_bytes(4, 'little') + placed[0]['reference'].to_bytes(4, 'little') + bytes(4)

    def index(self, physical, column, rows):
        tree = self.state['trees'][physical]
        assert len(tree['nodes']) == len(tree['leaves']) == 1
        page = tree['leaves'][0]
        before = self.original[page * PAGE:(page + 1) * PAGE]
        entries = list(tree['entries'])
        old_families = {self.table['values'][identity][column] for identity in self.table['rows']}
        final_families = set(old_families)
        for row, placed in zip(rows, self.new_rows, strict=True):
            value = row[0 if column == 'ID' else 1]
            entries.append(source.index_wire.key(value, placed['reference']))
            final_families.add(value)
        entries.sort()
        assert len(entries) == len(set(entries))
        prefix = source.number(before, 24)
        if len(entries) < 2:
            prefix = min(prefix, len(entries[0]) if entries else 0)
        else:
            common = 0
            for left, right in zip(entries[0], entries[-1]):
                if left != right:
                    break
                common += 1
            prefix = min(prefix, common)
        active = b''.join(entry if position == 0 else entry[prefix:]
                          for position, entry in enumerate(entries))
        assert len(active) <= 3616, 'This authentic corpus keeps both original root leaves fitted'
        expected = bytearray(before)
        expected[27:480] = bytes(453)
        used = 0
        for position, entry in enumerate(entries):
            used += len(entry) if position == 0 else len(entry) - prefix
            expected[27 + used // 8] |= 1 << (used % 8)
        put(expected, 2, 3616 - len(active))
        put(expected, 24, prefix)
        expected[480:480 + len(active)] = active
        self.bytes[page * PAGE:(page + 1) * PAGE] = expected
        offset = tree['history_at']
        historical = source.number(self.original, offset, 4)
        put(self.bytes, offset, historical + len(final_families - old_families), 4)


def derive(state, selected):
    rows = requests.rows(selected)
    image = Image(state)
    compressed = state['tables']['Items']['columns']['Memo']['compressed']
    records = []
    # Every final descriptor is known before the ordinary DATA phase begins.
    for identity, group, blob, memo in rows:
        encoded_memo = memo_bytes(memo, compressed)
        descriptors = [image.descriptor(0, blob), image.descriptor(1, encoded_memo)]
        body = record(identity, group, *descriptors)
        assert len(body) + 2 <= 4082
        records.append(body)
    lval_end = len(image.bytes)
    for body in records:
        image.new_rows.append(image.append(image.owner, body, False))
    for physical, column in [(0, 'ID'), (1, 'Group')]:
        image.index(physical, column, rows)
    put(image.bytes, image.table['definition'] * PAGE + 16, 6 + len(rows), 4)
    for scope in state['scopes'].values():
        for orphan in scope['orphan']:
            assert image.bytes[orphan['start']:orphan['end']] == image.original[orphan['start']:orphan['end']]
    for row in image.table['rows'].values():
        frame, chain = source.long_wire.final_frame(image.original, row)
        assert image.bytes[frame['start']:frame['end']] == row['raw']
    result = bytes(image.bytes)
    for requested, placed in zip(rows, image.new_rows, strict=True):
        identity, group, blob, memo = requested
        body = result[placed['start']:placed['end']]
        assert source.number(body, 0) == 4 and source.number(body, len(body) - 3) == 2
        assert int.from_bytes(body[2:6], 'little', signed=True) == identity
        assert bool(body[-1] & 2) == (group is not None)
        if group is not None:
            assert int.from_bytes(body[6:10], 'little', signed=True) == group
        for name, value in [('Blob', blob), ('Memo', memo)]:
            decoded = source.long_wire.long_value(result, body, image.table['columns'][name])['logical']
            if isinstance(decoded, dict):
                decoded = bytes.fromhex(decoded['binary_hex'])
            assert decoded == value
    for physical, column in [(0, 'ID'), (1, 'Group')]:
        original_tree = state['trees'][physical]
        root = original_tree['leaves'][0]
        _, _, entries = source.index_wire.tree(result, root, image.table['definition'])
        expected = list(original_tree['entries'])
        expected.extend(source.index_wire.key(row[0 if column == 'ID' else 1], placed['reference'])
                        for row, placed in zip(rows, image.new_rows, strict=True))
        assert entries == sorted(expected)
    return result, {'sha256': hashlib.sha256(result).hexdigest(), 'bytes': len(result),
                    'lval_phase_end': lval_end, 'allocations': image.allocations,
                    'attempts': image.events, 'rows': image.new_rows,
                    'chunks': image.chunk_rows, 'map_offsets': sorted(image.map_offsets)}
