"""Independent shared raw map primitives."""

PAGE = 4096


def cell(raw, metadata, number):
    start = metadata['table_page'] * PAGE + metadata['row_start']
    stop = metadata['table_page'] * PAGE + metadata['row_end']
    block = raw[start:stop]
    if block[0] == 0:
        relative = number - int.from_bytes(block[1:5], 'little')
        assert relative >= 0 and 5 + relative // 8 < len(block)
        offset = start + 5 + relative // 8
        bit = relative % 8
    else:
        assert block[0] == 1
        pointer, relative = divmod(number, 32736)
        assert 5 + 4 * pointer <= len(block)
        bitmap = int.from_bytes(block[1 + 4 * pointer:5 + 4 * pointer], 'little')
        assert 0 < bitmap < len(raw) // PAGE
        assert raw[bitmap * PAGE:bitmap * PAGE + 4] == b'\x05\x01\x00\x00'
        offset = bitmap * PAGE + 4 + relative // 8
        bit = relative % 8
    return offset, 1 << bit


def allocation(raw, metadata):
    start = metadata['table_page'] * PAGE + metadata['row_start']
    stop = metadata['table_page'] * PAGE + metadata['row_end']
    block = raw[start:stop]
    members = set()
    if block[0] == 0:
        base = int.from_bytes(block[1:5], 'little')
        for index, value in enumerate(block[5:]):
            for bit in range(8):
                if value & (1 << bit):
                    members.add(base + index * 8 + bit)
    else:
        assert block[0] == 1 and (len(block) - 1) % 4 == 0
        for slot in range((len(block) - 1) // 4):
            pointer = int.from_bytes(block[1 + slot * 4:5 + slot * 4], 'little')
            if pointer == 0:
                continue
            bitmap = raw[pointer * PAGE:(pointer + 1) * PAGE]
            assert bitmap[:4] == b'\x05\x01\x00\x00'
            for index, value in enumerate(bitmap[4:]):
                for bit in range(8):
                    if value & (1 << bit):
                        members.add(slot * 32736 + index * 8 + bit)
    return members
