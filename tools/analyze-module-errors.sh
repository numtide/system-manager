#!/usr/bin/env bash
set -euo pipefail
# Analyze error messages from NixOS module evaluations with system-manager
# This script captures actual error messages and aggregates them
#
# Usage: ./tools/analyze-module-errors.sh [MODULE_COUNT] [PARALLEL_JOBS]
#
# Arguments:
#   MODULE_COUNT   - Number of modules to test from module-list.nix (default: all modules)
#   PARALLEL_JOBS  - Number of parallel evaluation jobs (default: 10)
#
# Examples:
#   ./tools/analyze-module-errors.sh           # Test **all** modules with 10 jobs
#   ./tools/analyze-module-errors.sh 100       # Test 100 modules with 10 jobs
#   ./tools/analyze-module-errors.sh 100 20    # Test 100 modules with 20 jobs
#
# Output:
#   - Real-time progress as modules are evaluated
#   - Summary of successful vs failed modules
#   - Ranked list of most common missing options
#   - Ranked list of most common missing attributes
#   - Detailed error messages for all failures

MODULE_COUNT="${1:-0}"
PARALLEL_JOBS="${2:-10}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

export PROJECT_ROOT

# Get nixpkgs path from flake
NIXOS_MODULES_PATH=$(nix eval --raw --impure --expr "(builtins.getFlake \"$PROJECT_ROOT\").inputs.nixpkgs.outPath")/nixos/modules
export NIXOS_MODULES_PATH

echo "Prefetching flake inputs to avoid concurrent fetcher contention..."
nix flake metadata "$PROJECT_ROOT" >/dev/null

echo "Modules path: $NIXOS_MODULES_PATH"
echo "Testing first: $MODULE_COUNT modules"
echo "Parallel jobs: $PARALLEL_JOBS"
echo ""

TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT
export TEMP_DIR

ERRORS_FILE="$TEMP_DIR/errors.txt"
MISSING_OPTIONS_FILE="$TEMP_DIR/missing_options.txt"

# Function to test a single module
test_module() {
  local MODULE_PATH="$1"
  local INDEX="$2"
  local TOTAL="$3"

  local MODULE_NAME=$(basename "$MODULE_PATH")

  # Run nix eval once and capture both stdout and stderr
  local EVAL_OUTPUT
  local EVAL_EXIT_CODE

  EVAL_OUTPUT=$(nix eval --impure --expr "
    let
      flake = builtins.getFlake \"$PROJECT_ROOT\";
      nixpkgs = flake.inputs.nixpkgs;
      lib = nixpkgs.lib;
      system-manager = import \"$PROJECT_ROOT/nix/lib.nix\" { inherit nixpkgs lib; };
    in
      (system-manager.makeSystemConfig {
        modules = [
          (import \"$MODULE_PATH\")
          {
            nixpkgs.hostPlatform = \"x86_64-linux\";
          }
        ];
      }).config.environment
  " 2>&1)
  EVAL_EXIT_CODE=$?

  if [ $EVAL_EXIT_CODE -eq 0 ]; then
    echo "[$INDEX/$TOTAL] ✅ $MODULE_NAME"
    echo "SUCCESS: $MODULE_PATH"
  else
    echo "[$INDEX/$TOTAL] ❌ $MODULE_NAME"
    echo "FAILED: $MODULE_PATH"

    # Save error for analysis
    {
      echo "=== $MODULE_PATH ==="
      echo "$EVAL_OUTPUT"
      echo ""
    } >> "$TEMP_DIR/errors_${INDEX}.txt"

    # Extract missing options
    echo "$EVAL_OUTPUT" | grep -oP "option \`\K[^\']+" >> "$TEMP_DIR/missing_options_${INDEX}.txt" 2>/dev/null || true

    # Extract missing attributes
    echo "$EVAL_OUTPUT" | grep -oP "attribute '(\K[^']+)(?=' missing)" >> "$TEMP_DIR/missing_attributes_${INDEX}.txt" 2>/dev/null || true
  fi
}

export -f test_module

# Read module list from module-list.nix
mapfile -t MODULES < <(nix eval --impure --json --expr "
  let
    flake = builtins.getFlake \"$PROJECT_ROOT\";
    lib = flake.inputs.nixpkgs.lib;
    modules = import \"$NIXOS_MODULES_PATH/module-list.nix\";
  in
    if $MODULE_COUNT == 0 then
      modules
    else
      lib.take $MODULE_COUNT modules
" | jq -r '.[]')

echo "Found ${#MODULES[@]} modules to test"
echo ""

# Run tests in parallel
printf "%s\n" "${MODULES[@]}" | \
  nl -v 1 | \
  parallel --will-cite --colsep '\t' -j "$PARALLEL_JOBS" --line-buffer \
    test_module {2} {1} "${#MODULES[@]}" | \
  tee "$TEMP_DIR/results.txt"

RESULTS=$(cat "$TEMP_DIR/results.txt")

# Combine all error files
cat "$TEMP_DIR"/errors_*.txt > "$ERRORS_FILE" 2>/dev/null || touch "$ERRORS_FILE"
cat "$TEMP_DIR"/missing_options_*.txt > "$MISSING_OPTIONS_FILE" 2>/dev/null || touch "$MISSING_OPTIONS_FILE"
cat "$TEMP_DIR"/missing_attributes_*.txt > "$TEMP_DIR/missing_attributes.txt" 2>/dev/null || touch "$TEMP_DIR/missing_attributes.txt"

# Parse results from parallel output
SUCCESS=$(echo "$RESULTS" | grep -c "^SUCCESS:" || true)
EVAL_ERROR=$(echo "$RESULTS" | grep -c "^FAILED:" || true)
TOTAL=${#MODULES[@]}

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "SUMMARY"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Total:        $TOTAL"
echo "✅ Success:   $SUCCESS ($(( SUCCESS * 100 / TOTAL ))%)"
echo "❌ Failed:    $EVAL_ERROR"
echo ""

if [[ -f "$MISSING_OPTIONS_FILE" && -s "$MISSING_OPTIONS_FILE" ]]; then
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo "MOST COMMON MISSING OPTIONS"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo ""
  echo "Implementing these options would unlock the most modules:"
  echo ""
  sort "$MISSING_OPTIONS_FILE" | uniq -c | sort -rn | head -20 | while read count option; do
    printf "  %3d modules need: %s\n" "$count" "$option"
  done
  echo ""
fi

if [[ -f "$TEMP_DIR/missing_attributes.txt" && -s "$TEMP_DIR/missing_attributes.txt" ]]; then
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo "MOST COMMON MISSING ATTRIBUTES"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo ""
  echo "These attributes are missing:"
  echo ""
  sort "$TEMP_DIR/missing_attributes.txt" | uniq -c | sort -rn | head -20 | while read count attr; do
    printf "  %3d modules need: config.%s\n" "$count" "$attr"
  done
  echo ""
fi

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "DETAILED ERRORS"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [[ -f "$ERRORS_FILE" ]]; then
  echo "Detailed error messages have been saved to detailed_errors.txt"
  cp "$ERRORS_FILE" detailed_errors.txt
fi
