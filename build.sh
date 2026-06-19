#!/bin/bash
# Build the scheduler in release mode.
set -euo pipefail
cargo build --release
