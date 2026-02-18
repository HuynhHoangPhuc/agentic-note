#!/usr/bin/env bash
set -euo pipefail


TARGET="${1:-}"
if [[ -z "$TARGET" ]]; then
  echo "usage: $0 <target>"
  exit 1
fi

VERSION_TAG="${GITHUB_REF_NAME:-${VERSION_TAG:-unknown}}"
DIST_DIR="dist"
BIN_NAME="agentic-note"

mkdir -p "$DIST_DIR"

if [[ "$TARGET" == *"windows"* ]]; then
  BIN_PATH="target/${TARGET}/release/${BIN_NAME}.exe"
  ARCHIVE_NAME="${BIN_NAME}-${VERSION_TAG}-${TARGET}.zip"
  if [[ ! -f "$BIN_PATH" ]]; then
    echo "missing binary: $BIN_PATH"
    exit 1
  fi
  (cd "target/${TARGET}/release" && zip -q "${PWD}/../../../../${DIST_DIR}/${ARCHIVE_NAME}" "${BIN_NAME}.exe")
else
  BIN_PATH="target/${TARGET}/release/${BIN_NAME}"
  ARCHIVE_NAME="${BIN_NAME}-${VERSION_TAG}-${TARGET}.tar.gz"
  if [[ ! -f "$BIN_PATH" ]]; then
    echo "missing binary: $BIN_PATH"
    exit 1
  fi
  tar -C "target/${TARGET}/release" -czf "${DIST_DIR}/${ARCHIVE_NAME}" "${BIN_NAME}"
fi

ls -l "${DIST_DIR}/${ARCHIVE_NAME}"
