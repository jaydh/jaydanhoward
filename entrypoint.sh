#!/bin/bash

set -m

echo "Starting netdata"
netdata -d -u root -W "claim -token=$NETDATA_CLAIM_TOKEN rooms=a707fdee-792b-4e43-96b0-4a1a62771462 url=https://app.netdata.cloud" 


echo "Starting nginx"
service nginx start &

echo "Starting HTTP server"
./jaydanhoward
