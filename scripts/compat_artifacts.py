"""Root-scoped, fresh saved-artifact reads for engineering evidence tools.

This module executes no fixture processes and gives no stored content authority.
Protocol-specific expected hashes, models, Git identities and proofs stay with
its consumers. Files are reread on every pin/load; no cache or hash fallback is
introduced.
"""
import hashlib
import json
from pathlib import Path


class ArtifactStore:
    """Read artifacts confined to one explicit canonical filesystem root."""

    def __init__(self, root):
        self._root = Path(root).resolve()

    def path(self, value):
        """Resolve an in-root path, including symlinks, without requiring a file."""
        result = (self._root / value).resolve()
        result.relative_to(self._root)
        return result

    def pin(self, value):
        """Read current bytes and return the maintained path/size/SHA-256 shape."""
        target = self.path(value)
        data = target.read_bytes()
        return {
            'path': target.relative_to(self._root).as_posix(),
            'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest(),
        }

    def load(self, value):
        """Read and decode JSON with the existing standard-library semantics."""
        return json.loads(self.path(value).read_text())
