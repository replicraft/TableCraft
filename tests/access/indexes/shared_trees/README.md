# Shared metadata tree traversal

metadata.py walks already decoded page descriptions. Raw model and reference decoding remain separate owners, retaining their own validation and source policies.

`model.py`, `reference.py` and `integral.py` keep separate raw validation bodies.
They share only within their original model/reference families; callers retain
typed key decoding and numeric/header policy explicitly.
