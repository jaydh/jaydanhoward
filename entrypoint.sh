#!/bin/bash

set -m

echo "Starting HTTP server"
./jaydanhoward &

echo "Starting nginx"
service nginx start &

echo "Starting netdata"
netdata &

fg %1
