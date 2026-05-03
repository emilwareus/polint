# Quick Task 260503-lht: Fix release checksum paths for installer

**Date:** 2026-05-03
**Status:** Complete

## Goal

Fix the published release checksum files so `scripts/install.sh` can verify a
downloaded asset from its temporary download directory.

## Tasks

1. Change the workflow package step to write SHA-256 manifests from inside the
   `dist/` directory, so checksum entries contain only the archive filename.
2. Revalidate workflow YAML and shell syntax.
3. Push to `main`, wait for release publication, and retest the installer.
