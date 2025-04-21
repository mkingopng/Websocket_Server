#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Setting up Git hooks...${NC}"

# Make sure the hooks directory exists
mkdir -p .git/hooks

# Install the pre-commit hook
echo -e "Installing pre-commit hook..."
cp pre-commit-hook.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit

# Check if installation was successful
if [ -x .git/hooks/pre-commit ]; then
    echo -e "${GREEN}Pre-commit hook installed successfully!${NC}"
else
    echo -e "${RED}Failed to install pre-commit hook!${NC}"
    exit 1
fi

echo -e "${GREEN}Git hooks setup complete.${NC}"
echo -e "The pre-commit hook will now run automatically before each commit."
echo -e "It will check for code formatting, linting issues, and run tests."
exit 0 