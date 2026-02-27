#!/bin/bash
cd "$(git rev-parse --show-toplevel)"
git config core.hooksPath scripts/git-hooks
echo "Git hooks configured: scripts/git-hooks"
