"""Independent ordinary-parent and raw LVAL placement model, no engine calls."""
import hashlib

PAGE = 4096
HEADER = 14
SLOT_BYTES = 2
COUNT = 12
FREE = 2
OFFSET_MASK = 0x1fff
FORWARDED = 0x4000
HIDDEN = 0x8000
FOOTER = 9
INLINE_LIMIT = 64
EXTERNAL_ROW = 4076
CHAIN_PAYLOAD = 4072
MAX_SLOTS = 255


def number(raw, at, width=2):
    return int.from_bytes(raw[at:at + width], 'little')


def put(raw, at, value, width=2):
    raw[at:at + width] = value.to_bytes(width, 'little')


def frame(raw, page, slot):
    base = page * PAGE
    word = number(raw, base + HEADER + slot * SLOT_BYTES)
    start = base + (word & OFFSET_MASK)
    end = base + PAGE if slot == 0 else base + (number(raw, base + HEADER + (slot - 1) * SLOT_BYTES) & OFFSET_MASK)
    return word, start, end


def payload(raw, field):
    if not field['present'] or field['kind'] == 'NULL':
        return None
    wire = bytes.fromhex(field['wire_hex'])
    if field['kind'] == 'inline':
        value = wire[12:]
    else:
        parts = []
        for chunk in field['chunks']:
            body = raw[chunk['start']:chunk['end']]
            assert hashlib.sha256(body).hexdigest() == chunk['body_sha256']
            parts.append(body[4:] if field['kind'] == 'chain' else body)
        value = b''.join(parts)
    assert len(value) == field['encoded_bytes']
    assert hashlib.sha256(value).hexdigest() == field['encoded_sha256']
    return value


def cell(raw, description, page):
    start = description['row_start']
    end = description['row_end']
    kind = raw[start]
    if kind == 0:
        base = number(raw, start + 1, 4)
        relative = page - base
        assert 0 <= relative < (end - start - 5) * 8, 'Unrepresented inline map cell'
        return start + 5 + relative // 8, 1 << (relative % 8)
    assert kind == 1
    covered = (PAGE - 4) * 8
    pointer_at = start + 1 + page // covered * 4
    assert pointer_at + 4 <= end
    pointer = number(raw, pointer_at, 4)
    assert pointer and (pointer + 1) * PAGE <= len(raw)
    relative = page % covered
    return pointer * PAGE + 4 + relative // 8, 1 << (relative % 8)


def minimum_record(old, column, descriptor):
    assert number(old, 0) == 3 and number(old, len(old) - 3) == 2
    first = number(old, len(old) - 5)
    middle = number(old, len(old) - 7)
    end = number(old, len(old) - 9)
    assert first == 6 and first <= middle <= end <= len(old) - FOOTER
    values = [old[first:middle], old[middle:end]]
    values[0 if column == 'Memo' else 1] = descriptor if descriptor is not None else b''
    data = old[:first] + b''.join(values)
    length = max(len(old), len(data) + FOOTER)
    result = bytearray(old)
    result.extend(bytes(length - len(old)))
    result[:len(data)] = data
    footer = len(data).to_bytes(2, 'little')
    footer += (first + len(values[0])).to_bytes(2, 'little')
    footer += first.to_bytes(2, 'little') + (2).to_bytes(2, 'little')
    mask = old[-1]
    bit = 2 if column == 'Memo' else 4
    mask = mask | bit if descriptor is not None else mask & ~bit
    footer += bytes([mask])
    result[-FOOTER:] = footer
    return bytes(result)


class Image:
    def __init__(self, raw, state, definition):
        self.original = raw
        self.after = bytearray(raw)
        self.state = state
        self.definition = definition
        self.hints = {role: list(state['maps'][role + 'Hint']['file_members']) for role in ['Table', 'Memo', 'OLE']}
        self.events = []
        self.map_offsets = set()

    def bit(self, role, page, set_bit):
        offset, mask = cell(self.original, self.state['maps'][role], page)
        before = self.after[offset]
        self.after[offset] = before | mask if set_bit else before & ~mask
        self.map_offsets.add(offset)

    def allocate(self, role):
        page = len(self.after) // PAGE
        own, own_mask = cell(self.original, self.state['maps'][role + 'Owned'], page)
        free, free_mask = cell(self.original, self.state['maps']['GlobalFree'], page)
        assert not self.original[own] & own_mask and self.original[free] & free_mask
        raw = bytearray(PAGE)
        raw[:2] = bytes([1, 1])
        raw[4:8] = self.definition.to_bytes(4, 'little') if role == 'Table' else b'LVAL'
        put(raw, FREE, PAGE - HEADER)
        self.after.extend(raw)
        self.bit(role + 'Owned', page, True)
        self.bit(role + 'Hint', page, True)
        self.bit('GlobalFree', page, False)
        self.hints[role].append(page)
        self.events.append({'kind': 'EOF', 'role': role, 'page': page})
        return page

    def append(self, role, body):
        selected = None
        for page in sorted(self.hints[role], reverse=True):
            base = page * PAGE
            count = number(self.after, base + COUNT)
            free = number(self.after, base + FREE)
            if count < MAX_SLOTS and len(body) + SLOT_BYTES <= free:
                selected = page
                break
            self.bit(role + 'Hint', page, False)
            self.hints[role].remove(page)
            self.events.append({'kind': 'failed_candidate', 'role': role, 'page': page})
        page = self.allocate(role) if selected is None else selected
        base = page * PAGE
        count = number(self.after, base + COUNT)
        free = number(self.after, base + FREE)
        end = base + PAGE if not count else frame(self.after, page, count - 1)[1]
        start = end - len(body)
        assert start >= base + HEADER + (count + 1) * SLOT_BYTES
        self.after[start:end] = body
        word = start - base
        if role == 'Table':
            word |= HIDDEN
        put(self.after, base + HEADER + count * SLOT_BYTES, word)
        put(self.after, base + COUNT, count + 1)
        put(self.after, base + FREE, free - len(body) - SLOT_BYTES)
        self.events.append({'kind': 'append', 'role': role, 'page': page, 'slot': count,
                            'body_bytes': len(body), 'offset': start})
        return (page << 8) | count, start


def expected(raw, state, column, replacement, definition):
    current = payload(raw, state['fields'][column])
    if current == replacement:
        return raw, {'noop': True, 'events': [], 'whole_image_sha256': hashlib.sha256(raw).hexdigest()}
    image = Image(raw, state, definition)
    final = state['root_path'][-1]
    old = raw[final['start']:final['end']]
    if replacement is None:
        descriptor = None
        storage = 'NULL'
    elif len(replacement) <= INLINE_LIMIT:
        descriptor = (0x80000000 | len(replacement)).to_bytes(4, 'little') + bytes(8) + replacement
        storage = 'inline'
        if len(minimum_record(old, column, descriptor)) > PAGE - HEADER - SLOT_BYTES:
            assert replacement, 'Empty external value has no qualified representation'
            storage = 'single'
    else:
        descriptor = None
        storage = 'single' if len(replacement) <= EXTERNAL_ROW else 'chain'
    if storage in ['single', 'chain']:
        chunks = [replacement] if storage == 'single' else [replacement[at:at + CHAIN_PAYLOAD] for at in range(0, len(replacement), CHAIN_PAYLOAD)]
        references = []
        for part in chunks:
            body = bytes(4) + part if storage == 'chain' else part
            reference, offset = image.append(column, body)
            references.append((reference, offset))
        if storage == 'chain':
            for position, (_, offset) in enumerate(references):
                following = references[position + 1][0] if position + 1 < len(references) else 0
                put(image.after, offset, following, 4)
        tag = 0x40000000 if storage == 'single' else 0
        descriptor = (tag | len(replacement)).to_bytes(4, 'little') + references[0][0].to_bytes(4, 'little') + bytes(4)
    record = minimum_record(old, column, descriptor)
    if len(record) <= len(old):
        image.after[final['start']:final['end']] = record
        parent = 'fitted'
    else:
        reference, _ = image.append('Table', record)
        root = state['root_path'][0]
        put(image.after, root['start'], reference, 4)
        if not root['word'] & FORWARDED:
            image.after[root['start'] + 4:root['end']] = bytes(root['end'] - root['start'] - 4)
        put(image.after, root['page'] * PAGE + HEADER + root['slot'] * SLOT_BYTES, root['word'] | FORWARDED)
        parent = 'forwarded'
    # Old external chunks are opaque preserved storage, whether live or orphaned.
    for field in state['fields'].values():
        for chunk in field['chunks']:
            assert image.after[chunk['start']:chunk['end']] == raw[chunk['start']:chunk['end']]
            at = chunk['page'] * PAGE + HEADER + chunk['slot'] * SLOT_BYTES
            assert image.after[at:at + 2] == raw[at:at + 2]
    edits = []
    for offset in sorted(image.map_offsets):
        if image.after[offset] != raw[offset]:
            edits.append({'offset': offset, 'before': raw[offset], 'after': image.after[offset]})
    result = bytes(image.after)
    facts = {'noop': False, 'storage': storage, 'parent': parent, 'record_capacity_before': len(old),
             'record_capacity_after': len(record), 'events': image.events,
             'exact_map_byte_edits': edits, 'file_growth': len(result) - len(raw),
             'whole_image_sha256': hashlib.sha256(result).hexdigest(),
             'all_original_live_chunk_bodies_and_slots_preserved': True}
    return result, facts
