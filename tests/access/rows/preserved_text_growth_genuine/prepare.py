"""Derive immutable growth images from original bytes and explicit requests."""
import hashlib

from . import model


def derive_cases(raw, getter, cases, measured):
    """Yield images, proofs and reader observations without file or process I/O."""
    source_hash = hashlib.sha256(raw).hexdigest()
    getter_bytes = getter.encode('utf-8')
    getter_hash = hashlib.sha256(getter_bytes).hexdigest()
    batch = None
    for case in cases:
        model.require(case['header'] == raw[20], 'Explicit original header')
        model.require(case['source']['bytes'] == len(raw)
                      and case['source']['sha256'] == source_hash, 'Immutable original')
        model.require(case['sdk_stdout']['bytes'] == len(getter_bytes)
                      and case['sdk_stdout']['sha256'] == getter_hash, 'Original reader facts')
        image, proof, reader = model.construct(raw, getter, case, measured)
        if case['id'].endswith('-batch'):
            batch = image, reader
        if case['id'].endswith('-batch-reverse'):
            model.require(batch == (image, reader), 'Source-order batch identity')
        yield case['id'], image, proof, reader
