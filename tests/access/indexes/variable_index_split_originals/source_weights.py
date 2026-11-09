"""Exact independent finite weight/digest functions from accepted source model."""
from shared_keys.weights import expected_encodings as shared_expected_encodings
def expected_encodings(table, descending):
    """Bind this feature's source policy to shared independent mechanics."""
    return shared_expected_encodings(
        table,
        descending,
        truncate_key=truncate_key,
    )


def truncate_key(wire):
    """Bit-serial polynomial derivation from the reviewed entry-digest contract."""
    if len(wire) <= 510:
        return wire
    register = 0
    for byte in wire[508:-1]:
        for _ in range(8):
            high = register & 0x8000
            register = (register << 1) & 0xffff
            if high:
                register ^= 0x8005
        register ^= byte << 8
    register ^= wire[-1]
    return wire[:508] + register.to_bytes(2, 'little')
