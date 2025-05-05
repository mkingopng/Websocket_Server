#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Running pre-commit checks${NC}"

# Check if any Rust files will be committed
RUST_FILES=$(git diff --cached --name-only | grep -E '\.rs$')
if [ -z "$RUST_FILES" ]; then
    echo -e "${GREEN}No Rust files to check.${NC}"
    exit 0
fi

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

# 4. Run unit tests
echo "Running unit tests..."
cargo test --lib
UNIT_TEST_RESULT=$?

if [ $UNIT_TEST_RESULT -ne 0 ]; then
    echo -e "${RED}Unit tests failed. Please fix the issues before committing.${NC}"
    exit 1
else
    echo -e "${GREEN}Unit tests passed.${NC}"
fi

# 5. Run integration tests
echo "Running integration tests..."
cargo test integration::
INTEGRATION_TEST_RESULT=$?

if [ $INTEGRATION_TEST_RESULT -ne 0 ]; then
    echo -e "${RED}Integration tests failed. Please fix the issues before committing.${NC}"
    exit 1
else
    echo -e "${GREEN}Integration tests passed.${NC}"
fi

echo -e "${GREEN}All checks passed! Commit is ready to be created.${NC}"
exit 0 
