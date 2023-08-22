#!/bin/bash

set -m


echo "Starting nginx"
service nginx start &

echo "Starting netdata"
netdata  -W $NETDATA_CLAIM_TOKEN &

echo "Starting HTTP server"
./jaydanhoward 
