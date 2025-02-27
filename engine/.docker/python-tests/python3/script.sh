#!/bin/bash

# Exit on error
set -e
# Echo each command
set -x

# baml init -n requires a python file to auto-detect its a python project
touch main.py

baml init -n

# Run the command and write stdout and stderr to different files
baml test run > $CAPTURE_DIR/baml_test_stdout.log 2> $CAPTURE_DIR/baml_test_stderr.log

python -m baml_example_app > $CAPTURE_DIR/baml_example_stdout.log 2> $CAPTURE_DIR/baml_example_stderr.log

baml version --check --output json >$CAPTURE_DIR/baml_version_check.json
python3 <<EOF
import json, sys
checked_versions = json.load(open('$CAPTURE_DIR/baml_version_check.json'))
print(checked_versions)
assert all([g['current_version'][0].isdigit() for g in checked_versions['generators']]), "baml cli failed to parse package_version_command"
EOF
