#!/bin/bash
# Build, then generate both schedules for every test program under tests/.
set -euo pipefail

./build.sh

for test_dir in ./tests/*/
do
    ./run.sh "$test_dir/input.json" "$test_dir/simple.json" "$test_dir/pip.json"
done
