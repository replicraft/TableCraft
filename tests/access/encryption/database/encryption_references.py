"""Reviewed engineering artifacts; no runtime or downloaded code is committed."""

import hashlib
import os

from compare_union_types import JAR_SHA256

CRYPT_REWRITE_SHA256 = "6dbb05c767a15edd33fad0ca1d054767e71992748f2cba11d227bac00d4e648d"
BC_SHA256 = "2af190b300cbb0b35e248ccf5f4a06b6072030aeb3da7a98ec73abe5b4cb371f"


def rewrite_classpath(jar, crypt_jar, bc_jar):
    for artifact, expected in [
        (jar, JAR_SHA256),
        (crypt_jar, CRYPT_REWRITE_SHA256),
        (bc_jar, BC_SHA256),
    ]:
        with artifact.open("rb") as source:
            actual = hashlib.file_digest(source, "sha256").hexdigest()
        if actual != expected:
            raise ValueError(
                f"Unreviewed artifact {artifact.name}; rewrite/SID helpers require "
                "Jackcess 5.0.4, Jackcess Encrypt 5.0.2 and Bouncy Castle 1.86"
            )
    return os.pathsep.join(str(path) for path in [jar, crypt_jar, bc_jar])
