#!/bin/bash

set -m

echo "Starting nginx"
service nginx start

echo "Starting HTTP server"
./jaydanhoward &

fg %1
