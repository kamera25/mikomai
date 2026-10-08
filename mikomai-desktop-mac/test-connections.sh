#!/bin/sh
set -eu
APP="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
export MIKOMAI_WINDOW_CHECK_SOURCE="$APP/Tests/ConnectionInteractionChecks/ConnectionInteractionChecks.swift"
exec sh "$APP/test-chat-window.sh"
