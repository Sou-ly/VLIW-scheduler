#!/bin/bash
# Schedule a single program two ways.
#   ./run.sh <input.json> <loop_out.json> <pip_out.json>
# Produces a non-pipelined (loop) schedule and a software-pipelined (loop.pip)
# schedule for the same input program.
set -euo pipefail
cargo run --release -- "$1" "$2" --no_pip
cargo run --release -- "$1" "$3" --pip
