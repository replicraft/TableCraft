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

## Noto Emoji fallback

Unmodified Noto Emoji variable TrueType font, version 3.002, from Google's
official [font repository](https://github.com/google/fonts/tree/8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5/ofl/notoemoji).
The original `NotoEmoji[wght].ttf` bytes are stored locally as
`NotoEmoji-Variable.ttf`; the font metadata and glyphs are unchanged. Copyright
2013 Google LLC, distributed under the [SIL OFL 1.1](OFL-NotoEmoji.txt).
The [provenance record](noto-emoji-provenance.json) pins the source revision,
exact asset URLs, sizes and SHA-256 hashes for the font and notice.

This is a monochrome outline fallback compatible with the shared renderer;
it does not provide color emoji. It precedes egui's older emoji faces in both
proportional and monospace families, while retaining Selawik and existing Latin
defaults and the locale-specific CJK order. About exposes the source, credit and
complete license. No system font installation or font conversion is performed.
