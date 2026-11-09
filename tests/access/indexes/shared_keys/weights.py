"""Shared independent engineering mechanics with explicit feature bindings.

Callers retain their source/codec policy. No production encoder or observed
replacement is used to manufacture an expectation.
"""

def expected_encodings(
    table,
    descending,
    *,
    truncate_key,
):
    """Independent finite ASCII/segment oracle; no production encoder calls."""
    if table == 'BinaryOne':
        old = bytes.fromhex('80fffffffffffffffffe' if descending else '7f000000000000000001')
        segment = bytes([0 if descending else 255]) * 8 + bytes([9])
        final = (bytes([0 if descending else 255]) * 7
                 + bytes([255 if descending else 0, 248 if descending else 7]))
        new = bytes([128 if descending else 127]) + segment * 31 + final
        assert len(new) == 289
        return old, new
    modern = table == 'TextModern'
    primary_old = bytes.fromhex('0e02') if modern else bytes.fromhex('4a')
    primary_new = bytes.fromhex('0ea9') if modern else bytes.fromhex('78')

    def key(primary, count):
        payload = primary * count + bytes([1, 0])
        if descending:
            return bytes([128]) + bytes(byte ^ 255 for byte in payload) + bytes([0])
        return bytes([127]) + payload

    old = key(primary_old, 3)
    new = truncate_key(key(primary_new, 255))
    expected_length = 510 if modern else (259 if descending else 258)
    assert len(new) == expected_length
    return old, new
