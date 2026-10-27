"""Original-byte role overlay and a single typed EOF allocator; no native calls."""
PAGE = 4096
INLINE_WIDTH = 512
RECORD_BYTES = 69
BITMAP_PAGES = 32736


class Allocator:
    def __init__(self, before, result, facts, cell, decode):
        self.before = before
        self.result = result
        self.facts = facts
        self.cell = cell
        self.decode = decode
        self.cursor = len(before) // PAGE
        self.edits = {}
        self.roles = {}
        self.purposes = []
        self.states = {}

    def state(self, role):
        if role not in self.states:
            metadata = self.facts['maps'][role]
            start = metadata['table_page'] * PAGE + metadata['row_start']
            end = metadata['table_page'] * PAGE + metadata['row_end']
            raw = self.before[start:end]
            assert len(raw) == RECORD_BYTES and raw[0] in (0, 1)
            self.states[role] = {
                'start': start,
                'end': end,
                'raw': raw,
                'base': int.from_bytes(raw[1:5], 'little') if raw[0] == 0 else None,
                'members': set(self.decode(self.before, metadata)),
                'rewritten': False,
                'bitmap': None,
            }
        return self.states[role]

    def bit(self, role, number, setting):
        offset, mask = self.cell(self.before, self.facts['maps'][role], number)
        current = self.edits.get(offset, self.before[offset])
        assert bool(current & mask) != setting, (role, number, setting)
        self.edits[offset] = current | mask if setting else current & ~mask

    def record(self, state, after):
        assert len(after) == RECORD_BYTES
        for offset in range(state['start'], state['end']):
            self.edits.pop(offset, None)
        for relative, value in enumerate(after):
            self.edits[state['start'] + relative] = value
        state['rewritten'] = True

    def delta(self, role, number, setting):
        changes = self.roles.setdefault(role, {'added': [], 'removed': []})
        changes['added' if setting else 'removed'].append(number)

    def reserve(self, kind, **identity):
        number = self.cursor
        self.cursor += 1
        assert len(self.result) == number * PAGE
        self.result.extend(bytes(PAGE))
        self.purposes.append({'page': number, 'kind': kind, **identity})
        self.bit('GlobalFree', number, False)
        self.delta('GlobalFree', number, False)
        return number

    def remove_hint(self, role, number):
        state = self.state(role)
        assert state['raw'][0] == 0 and not state['rewritten']
        assert number == max(state['members'])
        self.bit(role, number, False)
        state['members'].remove(number)
        self.delta(role, number, False)

    def data(self, table):
        definition = self.facts['tables'][table]['definition']
        number = self.reserve('DATA', definition=definition)
        self.bit(table + ':Owned', number, True)
        self.delta(table + ':Owned', number, True)
        self.add(table + ':Hint', number, 'CANDIDATE_BITMAP', definition=definition)
        return number

    def index(self, role, physical):
        metadata = self.facts['maps'][role]
        usage = (metadata['table_page'] << 8) | metadata['decoded']['map_slot']
        number = self.reserve('INDEX', physical=physical, usage=usage)
        self.add(role, number, 'INDEX_BITMAP', physical=physical, usage=usage)
        return number

    def add(self, role, number, bitmap_kind, **identity):
        state = self.state(role)
        assert number not in state['members']
        state['members'].add(number)
        self.delta(role, number, True)
        if state['bitmap'] is not None:
            bitmap = state['bitmap']
            assert number != bitmap and number < BITMAP_PAGES
            at = bitmap * PAGE + 4 + number // 8
            assert self.result[at] & (1 << (number % 8)) == 0
            self.result[at] |= 1 << (number % 8)
            return
        if state['raw'][0] == 1:
            self.bit(role, number, True)
            return
        base = state['base']
        if base <= number < base + INLINE_WIDTH:
            if state['rewritten']:
                self.inline(state, base)
            else:
                self.bit(role, number, True)
            return
        aligned = min(state['members']) // 8 * 8
        span = max(state['members']) - aligned + 1
        if span < INLINE_WIDTH:
            self.inline(state, aligned)
            state['base'] = aligned
            return
        metadata = self.reserve(bitmap_kind, **identity)
        assert metadata not in state['members']
        assert max(state['members']) < BITMAP_PAGES
        bitmap = bytearray(PAGE)
        bitmap[:4] = b'\x05\x01\x00\x00'
        for member in state['members']:
            bitmap[4 + member // 8] |= 1 << (member % 8)
        self.result[metadata * PAGE:(metadata + 1) * PAGE] = bitmap
        pointer = bytearray(RECORD_BYTES)
        pointer[0] = 1
        pointer[1:5] = metadata.to_bytes(4, 'little')
        self.record(state, pointer)
        state['bitmap'] = metadata

    def inline(self, state, base):
        row = bytearray(RECORD_BYTES)
        row[1:5] = base.to_bytes(4, 'little')
        assert base % 8 == 0
        for member in state['members']:
            relative = member - base
            assert 0 <= relative < INLINE_WIDTH
            row[5 + relative // 8] |= 1 << (relative % 8)
        self.record(state, row)

    def finish(self):
        assert len(self.result) == self.cursor * PAGE
        original_eof = len(self.before) // PAGE
        assert [purpose['page'] for purpose in self.purposes] == list(range(original_eof, self.cursor))
        data = {item['page'] for item in self.purposes if item['kind'] == 'DATA'}
        indexes = {item['page'] for item in self.purposes if item['kind'] == 'INDEX'}
        metadata = {item['page'] for item in self.purposes if item['kind'].endswith('BITMAP')}
        assert data.isdisjoint(indexes | metadata) and indexes.isdisjoint(metadata)
        for role, changes in self.roles.items():
            additions = set(changes['added'])
            if role.endswith(':Owned') or role.endswith(':Hint'):
                assert additions <= data
            elif ':Index:' in role:
                assert additions <= indexes
