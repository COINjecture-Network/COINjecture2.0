#!/bin/bash
# =============================================================================
# COINjecture Network B - Scenario Test Runner (Shell Wrapper)
# =============================================================================
# Usage:
#   ./tests/harness/run-scenarios.sh [scenario] [--keep-running]
#
# Scenarios:
#   cold-start      - Start from genesis, reach consensus
#   join-late       - Late joiner catch-up test
#   partition-heal  - Network partition and recovery
#   forced-fork     - Deliberate fork and reorg
#   adversarial     - Adversarial peer conditions
#   all             - Run all scenarios
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCENARIO="${1:-all}"
EXTRA_ARGS="${@:2}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}  COINjecture Network B - Test Harness${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo ""

# Check dependencies
command -v docker >/dev/null 2>&1 || { echo -e "${RED}❌ Docker required but not installed${NC}"; exit 1; }
command -v python3 >/dev/null 2>&1 || command -v python >/dev/null 2>&1 || { echo -e "${RED}❌ Python 3 required but not installed${NC}"; exit 1; }

# Get Python command
PYTHON_CMD="python3"
command -v python3 >/dev/null 2>&1 || PYTHON_CMD="python"

# Check requests module
$PYTHON_CMD -c "import requests" 2>/dev/null || {
    echo -e "${YELLOW}📦 Installing requests module...${NC}"
    pip install requests
}

# Create results directory
mkdir -p "$SCRIPT_DIR/results"
mkdir -p "$SCRIPT_DIR/keys/bootnode"
mkdir -p "$SCRIPT_DIR/keys/node-a"
mkdir -p "$SCRIPT_DIR/keys/node-b"
mkdir -p "$SCRIPT_DIR/keys/node-c"
mkdir -p "$SCRIPT_DIR/keys/node-d"
mkdir -p "$SCRIPT_DIR/keys/node-e"
mkdir -p "$SCRIPT_DIR/keys/node-f"

echo -e "${GREEN}🚀 Running scenario: ${SCENARIO}${NC}"
echo ""

# Run the Python scenario runner
cd "$SCRIPT_DIR/../.."
$PYTHON_CMD tests/harness/scenario_runner.py "$SCENARIO" $EXTRA_ARGS
EXIT_CODE=$?

echo ""
if [ $EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  ✅ ALL TESTS PASSED${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
else
    echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}  ❌ SOME TESTS FAILED${NC}"
    echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
fi

exit $EXIT_CODE
