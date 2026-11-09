# Shared split action inspection

`actions.py` owns common physical index inspection, complete map membership and
before/after action analysis. Each caller explicitly supplies its typed source
model and related inspection policy. Independent first-party raw sources and
pinned cached action facts verify the extraction; no production writer or
encoder is used to construct expectations.

`candidates.py` owns common typed candidate-tree traversal with caller-bound key
validation. `render.py` owns common independent page framing with caller-bound
prefix and field encoders. These are not production encoders and do not replace
reference/source qualification.
