#!/bin/sh
# Resolve the selected package before launching so relative resources remain usable.
set -eu
personal_root="$HOME/.local/opt/codex-personal"
release=$(CDPATH= cd -P "$personal_root/current" && pwd -P) || {
  printf '%s\n' 'No selected personal Codex package.' >&2
  exit 1
}
case "$release/" in
  "$personal_root/releases/"*) ;;
  *) printf '%s\n' 'Personal Codex current must resolve inside releases.' >&2; exit 1 ;;
esac
[ -x "$release/bin/codex" ] && [ -f "$release/codex-package.json" ] || {
  printf '%s\n' 'Selected personal Codex package is incomplete.' >&2
  exit 1
}
# This override also selects the embedded server, leaving the shared daemon intact.
exec "$release/bin/codex" -c check_for_update_on_startup=false "$@"
