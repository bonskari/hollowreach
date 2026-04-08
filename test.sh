#!/bin/bash
# Safe test runner — NEVER touches the real display.
# Unsets WAYLAND_DISPLAY to prevent Bevy from using the real Wayland session.
unset WAYLAND_DISPLAY
unset DISPLAY
exec xvfb-run -a -s "-screen 0 1280x720x24" "$@"
