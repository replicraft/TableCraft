"""Original integer/index model of the WHATWG Shift_JIS token algorithms.

Reference files are read as private data, never embedded in this module. The
Microsoft mapping is a separate historical comparison, not the encoder oracle.
"""


def read_index(path):
    entries = {}
    for line in path.read_text().splitlines():
        content = line.split("#", 1)[0].strip()
        if content:
            pointer, codepoint, *_ = content.split()
            entries[int(pointer)] = int(codepoint, 16)
    return entries


def read_microsoft(path):
    entries = {}
    for line in path.read_text().splitlines():
        content = line.split("#", 1)[0].strip()
        fields = content.split()
        if len(fields) >= 2:
            token = int(fields[0], 16)
            entries[token.to_bytes(1 if token <= 255 else 2, "big")] = int(fields[1], 16)
    return entries


def decode(token, index):
    if len(token) == 1:
        byte = token[0]
        if byte <= 0x80:
            return byte
        if 0xa1 <= byte <= 0xdf:
            return 0xff61 + byte - 0xa1
        return None
    if len(token) != 2:
        return None
    lead, trail = token
    if not (0x81 <= lead <= 0x9f or 0xe0 <= lead <= 0xfc):
        return None
    if not (0x40 <= trail <= 0x7e or 0x80 <= trail <= 0xfc):
        return None
    lead_offset = 0x81 if lead < 0xa0 else 0xc1
    trail_offset = 0x40 if trail < 0x7f else 0x41
    pointer = (lead - lead_offset) * 188 + trail - trail_offset
    if 8836 <= pointer <= 10715:
        return 0xe000 + pointer - 8836
    return index.get(pointer)


def reverse_index(index):
    reverse = {}
    for pointer, codepoint in sorted(index.items()):
        if not 8272 <= pointer <= 8835:
            reverse.setdefault(codepoint, pointer)
    return reverse


def encode(codepoint, index, reverse):
    if codepoint <= 0x80:
        token = bytes([codepoint])
    elif codepoint == 0xa5:
        token = b"\\"
    elif codepoint == 0x203e:
        token = b"~"
    elif 0xff61 <= codepoint <= 0xff9f:
        token = bytes([codepoint - 0xff61 + 0xa1])
    else:
        selected = 0xff0d if codepoint == 0x2212 else codepoint
        pointer = reverse.get(selected)
        if pointer is None:
            return None
        lead, trail = divmod(pointer, 188)
        token = bytes([
            lead + (0x81 if lead < 0x1f else 0xc1),
            trail + (0x40 if trail < 0x3f else 0x41),
        ])
    # TableCraft's stricter Unicode round-trip policy excludes best-fit aliases.
    return token if decode(token, index) == codepoint else None


def decoder_cases(index):
    for byte in range(256):
        token = bytes([byte])
        yield token, decode(token, index)
    leads = [*range(0x81, 0xa0), *range(0xe0, 0xfd)]
    for lead in leads:
        for trail in [*range(0x40, 0x7f), *range(0x80, 0xfd)]:
            token = bytes([lead, trail])
            codepoint = decode(token, index)
            if codepoint is not None:
                yield token, codepoint


def encoder_cases(index):
    reverse = reverse_index(index)
    candidates = set(index.values())
    candidates.update(range(0x81))
    candidates.update(range(0xff61, 0xffa0))
    candidates.update([0xa5, 0x203e, 0x2212, 0xe000, 0xe757, 0x1f600])
    for codepoint in sorted(candidates):
        yield codepoint, encode(codepoint, index, reverse)


def goldens(index):
    reverse = reverse_index(index)
    assert decode(bytes.fromhex("93fa"), index) == 0x65e5
    assert decode(bytes.fromhex("967b"), index) == 0x672c
    assert decode(bytes.fromhex("f040"), index) == 0xe000
    assert decode(bytes.fromhex("f9fc"), index) == 0xe757
    assert decode(bytes.fromhex("80"), index) == 0x80
    assert decode(bytes.fromhex("817f"), index) is None
    assert encode(0x65e5, index, reverse) == bytes.fromhex("93fa")
    assert encode(0x672c, index, reverse) == bytes.fromhex("967b")
    for codepoint in [0xa5, 0x203e, 0x2212, 0xe000, 0x1f600]:
        assert encode(codepoint, index, reverse) is None
