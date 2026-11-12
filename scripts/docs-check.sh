#!/bin/sh
# The plan names this wrapper; the work lives in the Python script.
set -e
cd "$(dirname "$0")/.."
exec python3 scripts/docs-check.py "$@"
