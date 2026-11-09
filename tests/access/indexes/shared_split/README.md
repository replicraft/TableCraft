# Shared split action inspection

`actions.py` owns common physical index inspection, complete map membership and
before/after action analysis. Each caller explicitly supplies its typed source
model and related inspection policy. Independent first-party raw sources and
pinned cached action facts verify the extraction; no production writer or
encoder is used to construct expectations.
