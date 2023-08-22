#!/bin/bash

set -m

echo "Starting HTTP server"
./jaydanhoward &

echo "Starting nginx"
service nginx start &

fg %1

