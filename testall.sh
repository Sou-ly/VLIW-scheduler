#!/bin/bash
# Check the generated schedules against the reference outputs for every test.
# Run ./runall.sh first to produce simple.json / pip.json in each test folder.
set -uo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
RESET='\033[0m'

for test_dir in ./tests/*/
do
    loop_passed=false
    loop_color=$RED
    for simple_ref in "${test_dir}"/simple_ref*.json
    do
        out="$(python compare.py --loop "${test_dir}/simple.json" --refLoop "${simple_ref}")"
        if [[ "$(echo "$out" | head -n 1)" == *"PASSED"* ]]; then
            loop_passed=true
            loop_color=$GREEN
        fi
    done

    pip_passed=false
    pip_color=$RED
    for pip_ref in "${test_dir}"/pip_ref*.json
    do
        out="$(python compare.py --pip "${test_dir}/pip.json" --refPip "${pip_ref}")"
        if [[ "$(echo "$out" | head -n 1)" == *"PASSED"* ]]; then
            pip_passed=true
            pip_color=$GREEN
        fi
    done

    printf '%s' "$(cat "${test_dir}/desc.txt")"
    printf "\n  loop: ${loop_color}${loop_passed}${RESET}   loop.pip: ${pip_color}${pip_passed}${RESET}\n\n"
done
