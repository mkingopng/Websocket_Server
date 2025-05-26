#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Running pre-commit checks${NC}"

# Check if any Rust files exist (either staged or in working directory)
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    # If we're in a git repo, check staged files first, then all Rust files
    RUST_FILES=$(git diff --cached --name-only | grep -E '\.rs$')
    if [ -z "$RUST_FILES" ]; then
        # No staged files, check if any Rust files exist in the project
        RUST_FILES=$(find . -name "*.rs" -not -path "./target/*" | head -1)
    fi
else
    # Not in a git repo, just check for Rust files
    RUST_FILES=$(find . -name "*.rs" -not -path "./target/*" | head -1)
fi

if [ -z "$RUST_FILES" ]; then
    echo -e "${RED}No Rust files found to check.${NC}"
    exit 1
fi

echo -e "${GREEN}Found Rust files to check.${NC}"

# 1. Run cargo fmt to check formatting
echo "Checking code formatting..."
cargo fmt --all -- --check
FMT_RESULT=$?

if [ $FMT_RESULT -ne 0 ]; then
    echo -e "${RED}Formatting check failed. Please run 'cargo fmt --all' before committing.${NC}"
    exit 1
else
    echo -e "${GREEN}Formatting check passed.${NC}"
fi

# 2. Run cargo clippy to check for linting issues
echo "Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings
CLIPPY_RESULT=$?

if [ $CLIPPY_RESULT -ne 0 ]; then
    echo -e "${RED}Clippy check failed. Please fix the issues before committing.${NC}"
    exit 1
else
    echo -e "${GREEN}Clippy check passed.${NC}"
fi

# 3. Run cargo check to verify compilation
echo "Checking compilation..."
cargo check --all-targets --all-features
CHECK_RESULT=$?

if [ $CHECK_RESULT -ne 0 ]; then
    echo -e "${RED}Compilation check failed. Please fix the issues before committing.${NC}"
    exit 1
else
    echo -e "${GREEN}Compilation check passed.${NC}"
fi

# 4. Run all tests
echo "Running tests..."
cargo test
TEST_RESULT=$?

if [ $TEST_RESULT -ne 0 ]; then
    echo -e "${RED}Tests failed. Please fix the issues before committing.${NC}"
    exit 1
else
    echo -e "${GREEN}Tests passed.${NC}"
fi

echo -e "${GREEN}All checks passed!${NC}"
exit 0 
