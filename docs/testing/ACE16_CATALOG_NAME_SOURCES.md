# Catalog-name encoding sources

Four independent header-5 originals distinguish stored catalog Text from raw
definition names. They reuse the canonical V2 nine-page envelope, reserialize
the eight catalog DATA records directly, and retain all unrelated bytes. They
do not call a creator or use repaired producer output as an expectation.

Three sources store table names beginning U+FEFF: one unit, seven mixed Unicode
units and the full 64-unit boundary. Their literal `FF FE 00` plus UTF-16 payloads
occupy five, seventeen and 131 bytes. Directory/free fields and the variable
footer describe those actual lengths. Seven original catalog records and their
directory words remain exact. The fourth source keeps `Items` and changes only
ten raw definition-name bytes to a column spelling containing U+FEFF/U+0100.
This control remains readable without applying catalog compression rules.

The first native gate passed all four sources in 0.01 seconds, including exact
catalog/schema names and full insert-source metadata ownership. Export passed
for all four originals. All four first actual SDK reads passed using the unchanged
canonical V2 getter/classes; saved verification matched every complete image and
full observation. An earlier driver preflight referenced the wrong authority
key and made zero SDK calls. Its empty observation directory and premature
saved-check FileNotFound failure are retained separately; no source or getter
was changed. The ordinary hash anchor, strict and format checks passed.
No repair producer was invoked during this source phase. These observations do not establish native Access/Windows,
ordinary imported-file save or macro eligibility.

The [source result](results/ace16-catalog-name-source-2026-10-31.json) records exact inputs, observations and retained
failed attempts separately from production creation and native Access qualification.
