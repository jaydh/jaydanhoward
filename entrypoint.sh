#!/bin/bash

set -m


echo "Starting nginx"
service nginx start &

echo "Starting netdata"
netdata &

echo "Starting HTTP server"
./jaydanhoward 
