# Shared long-value source decoding

`source.py` owns common tagged-getter parsing and raw LVAL inventory traversal.
Each caller explicitly supplies its independent label/numeric decoder and page
size. Source-specific row, catalog, property, map and allocation qualification
stays in the bootstrap, map-growth and external-long-value feature models.

The helper reads retained raw evidence; it never uses a production encoder or
observed replacement to invent expected data.
