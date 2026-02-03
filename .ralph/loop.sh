#!/bin/bash
# Ralph Loop - Enriched UI Wrapper
# Reference: https://github.com/ghuntley/how-to-ralph-wiggum
#
# This wrapper script launches the enriched Python UI for the Ralph Loop.
# It checks for dependencies and forwards all arguments to enriched_loop.py.
#
# Usage: ./loop.sh [--codex|--cursor|--cursor-*] [plan] [max_iterations]
# Examples:
#   ./loop.sh              # Build mode with Claude (default)
#   ./loop.sh --codex      # Build mode with Codex
#   ./loop.sh --cursor     # Build mode with Cursor Agent
#   ./loop.sh --cursor-codex        # Cursor Agent with Codex model
#   ./loop.sh --cursor-grok         # Cursor Agent with Grok model
#   ./loop.sh --cursor-claude-sonnet # Cursor Agent with Claude Sonnet
#   ./loop.sh --cursor-claude-opus  # Cursor Agent with Claude Opus
#   ./loop.sh --cursor-gemini       # Cursor Agent with Gemini model
#   ./loop.sh --codex 20   # Build mode with Codex, max 20 iterations
#   ./loop.sh 20           # Build mode with Claude, max 20 iterations
#   ./loop.sh plan         # Plan mode with Claude
#   ./loop.sh plan 5       # Plan mode with Claude, max 5 iterations

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYTHON_SCRIPT="$SCRIPT_DIR/loop.py"

python3 -m venv .venv && . .venv/bin/activate

# Check if Python 3 is available
if ! command -v python3 &> /dev/null; then
    echo "Error: Python 3 is required but not installed."
    echo "Install Python 3 from: https://www.python.org/downloads/"
    exit 1
fi

# Check if rich library is installed
if ! python3 -c "import rich" &> /dev/null; then
    pip3 install rich
fi

# Check if the Python script exists
if [ ! -f "$PYTHON_SCRIPT" ]; then
    echo "Error: enriched_loop.py not found at $PYTHON_SCRIPT"
    exit 1
fi

# Make the Python script executable (if it isn't already)
chmod +x "$PYTHON_SCRIPT"

# Forward all arguments to the Python script
exec python3 "$PYTHON_SCRIPT" "$@"
