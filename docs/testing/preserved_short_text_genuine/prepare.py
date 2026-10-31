"""Pure sequential preparation from explicit original bytes and frozen cases."""
import hashlib

from preserved_int32_genuine import framing as raw_framing
from . import framing, model


def derive_cases(raw, getter, header, cases):
    """Yield independent immutable images/proofs/readers; perform no file I/O.

    Each source is qualified before its cases. Callers own persistence and any
    external source authorization; results grant no native write permission.
    """
    framing.inspect_original(raw, getter, header, raw_framing)
    source_hash = hashlib.sha256(raw).hexdigest()
    getter_bytes = getter.encode('utf-8')
    getter_hash = hashlib.sha256(getter_bytes).hexdigest()
    for case in cases:
        framing.require(case['header'] == header, 'Explicit source header')
        framing.require(case['source']['bytes'] == len(raw)
                        and case['source']['sha256'] == source_hash, 'Exact immutable original')
        framing.require(case['sdk_stdout']['bytes'] == len(getter_bytes)
                        and case['sdk_stdout']['sha256'] == getter_hash, 'Exact original observations')
        image, proof, reader = model.construct(raw, getter, header, case)
        yield case['id'], image, proof, reader
