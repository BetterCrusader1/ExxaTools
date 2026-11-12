#!/usr/bin/env python3
"""Fails when a command written in the docs no longer exists in the binary."""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BINARY = ROOT / "target/release/exxa"
DOCS = ["README.md", "ROADMAP.md"]


def usage() -> set[str]:
    out = subprocess.run([str(BINARY)], capture_output=True, text=True, check=False)
    text = out.stdout or out.stderr
    start = text.find("{")
    payload = json.loads(text[start:]) if start >= 0 else {}
    verbs = set()
    for line in payload.get("usage", []):
        parts = line.split()
        if len(parts) > 1:
            verbs.add(parts[1])
    return verbs


def documented() -> dict[str, str]:
    found: dict[str, str] = {}
    pattern = re.compile(r"^\s*(?:\S*/)?exxa\s+([a-z][a-z-]*)", re.MULTILINE)
    for name in DOCS:
        text = (ROOT / name).read_text()
        for match in pattern.finditer(text):
            found.setdefault(match.group(1), name)
    return found


def main() -> int:
    if not BINARY.is_file():
        print(f"docs-check: {BINARY} is missing, run cargo build --release first")
        return 1
    verbs = usage()
    missing = {verb: doc for verb, doc in documented().items() if verb not in verbs}
    if missing:
        for verb, doc in sorted(missing.items()):
            print(f"docs-check: '{verb}' in {doc} is not a command of the binary")
        return 1
    print(f"docs-check: {len(documented())} documented commands all exist")
    return 0


if __name__ == "__main__":
    sys.exit(main())
