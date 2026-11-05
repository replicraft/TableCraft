# Selawik regular

Unmodified `selawk.ttf` from Microsoft's official [Selawik 1.01 release](https://github.com/microsoft/Selawik/releases/tag/1.01), asset `Selawik_Release.zip`.

Copyright 2015 Microsoft Corporation. Reserved Font Name: Selawik. Distributed
under the [SIL Open Font License 1.1](OFL-Selawik.txt). The embedded font is part of the
shared UI theme; the host provides an About/licenses view. Egui's default fonts
remain as fallbacks. No font conversion or glyph modification was performed.

SHA-256 (`selawk.ttf`): `e9d98518d8ac2817782a9a382430463a2e0793ea68350b695bb727d9a830ee1c`.

## Noto CJK fallbacks

Unmodified Noto Sans JP, KR and SC Regular regional subset OTFs, version 2.004,
from the official [Noto CJK repository](https://github.com/notofonts/noto-cjk).
Copyright 2014–2021 Adobe (http://www.adobe.com/), retained in each font's name
metadata. Distributed under the [SIL Open Font License 1.1](OFL-NotoSansCJK.txt).
Exact immutable source URLs, byte sizes and SHA-256 hashes are recorded in
[the asset provenance](noto-sans-cjk-provenance.json).

Selawik remains first for proportional UI text. Japanese and Korean UI locales
prefer their respective CJK face; other locales prefer Simplified Chinese before
the other CJK fallbacks. The SQL editor retains its monospace Latin face, with
the same CJK fallback coverage. No system font discovery, glyph modification,
conversion or database-locale inference is performed.
