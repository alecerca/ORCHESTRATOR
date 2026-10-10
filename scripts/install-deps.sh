#!/usr/bin/env bash
#
# Alias kept for the feature document (`odd/tasks/orquestador-dashboard.md`),
# which refers to `scripts/install-deps.sh`. The canonical name is
# `scripts/setup-system-deps.sh` (SPEC §3). Both print the same instructions.
set -euo pipefail
exec "$(dirname "$0")/setup-system-deps.sh" "$@"
