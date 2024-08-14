#!/bin/bash

set -m

echo "Starting HTTP server"
./jaydanhoward &

fg %1
