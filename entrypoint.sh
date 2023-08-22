#!/bin/bash

set -m


echo "Starting nginx"
service nginx start &

echo "Starting netdata"
netdata -W "claim -token=$NETDATA_CLAIM_TOKEN rooms=jaydanhoward url=https://app.netdata.cloud" &

echo "Starting HTTP server"
./jaydanhoward 
