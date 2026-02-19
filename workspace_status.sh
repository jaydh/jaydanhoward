#!/bin/bash
echo "STABLE_GIT_COMMIT_SHA $(git rev-parse HEAD 2>/dev/null || echo 'unknown')"
