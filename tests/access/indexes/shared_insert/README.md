# Shared raw insertion mechanics

`derive.py` owns the common source/request-to-image algorithm used by the
variable, GUID tuple and decimal index insertion models. Callers explicitly
supply their independently authored codecs, framing helpers and resource ceilings.
It never consumes a production encoder or observed AFTER to construct an image.

The feature constructors keep their public domain policies. Shared mechanics do
not broaden the finite source profiles or native writer admission.
