#!/usr/bin/env python3
"""Verify ordered native rollback operations; live registration/hash checks stay in PowerShell."""
import argparse
import json
from pathlib import Path
import re
import sys

PAYLOADS = ("tablecraft-desktop.exe", "LICENSE", "ThirdPartyLicenses.txt", "RELEASE.json")
MAX_LOG_BYTES = 16 * 1024 * 1024


def verify(text, installation, old_product):
    if not re.fullmatch(r"\{[0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}\}", old_product):
        raise ValueError("Expected the original native product identity")

    def position(pattern, start=0):
        match = re.search(pattern, text[start:], re.IGNORECASE)
        if match is None:
            raise ValueError("Native log lacks the required ordered rollback operation")
        return start + match.end()

    removal = position(r"Action ended [^\r\n]*: RemoveExistingProducts\. Return value 1\.")
    before_removal = text[:removal]
    if "Executing op: ProductUnpublish(" not in before_removal:
        raise ValueError("Original product was not unpublished before the failure")
    for name in PAYLOADS:
        if re.search(r"Executing op: FileRemove\(,FileName=" + re.escape(name) + r",", before_removal) is None:
            raise ValueError("Original payload removal was not recorded")

    failed = position(r"Action ended [^\r\n]*: InstallFinalize\. Return value 3\.", removal)
    rollback = position(r"Executing op: Header\([^\r\n]*ScriptType=2,", failed)
    restored_product = position(
        r"Executing op: ProductInfo\(ProductKey=" + re.escape(old_product) + r",", rollback
    )
    rollback_end = position(r"Action ended [^\r\n]*: INSTALL\. Return value 3\.", restored_product)
    restoration = text[restored_product:rollback_end]
    destination = installation.rstrip("\\/") + "\\"
    for name in PAYLOADS:
        operation = (
            r"Executing op: FileCopy\(SourceName=[^,\r\n]+\.rbf,,DestName="
            + re.escape(destination + name)
            + r","
        )
        if re.search(operation, restoration, re.IGNORECASE) is None:
            raise ValueError("Native rollback did not restore every exact payload from its backup")
    return {
        "old_product_removed": True,
        "install_finalize_failed": True,
        "native_rollback_script_executed": True,
        "original_product_restored": True,
        "exact_payload_backups_restored": list(PAYLOADS),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--log", required=True, type=Path)
    parser.add_argument("--installation", required=True)
    parser.add_argument("--old-product", required=True)
    parser.add_argument("--evidence", required=True, type=Path)
    args = parser.parse_args()
    if args.log.stat().st_size > MAX_LOG_BYTES:
        raise ValueError("Native rollback log exceeds the evidence bound")
    evidence = verify(args.log.read_text(encoding="utf-16"), args.installation, args.old_product)
    with args.evidence.open("x", encoding="utf-8") as output:
        json.dump(evidence, output, indent=2)
        output.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (OSError, UnicodeError, ValueError) as error:
        print(f"Native MSI rollback verification failed: {error}", file=sys.stderr)
        raise SystemExit(1)
