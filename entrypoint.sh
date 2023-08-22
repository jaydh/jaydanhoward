#!/bin/bash

set -m

echo "Starting netdata"
/tmp/netdata-kickstart.sh --claim-token $NETDATA_CLAIM_TOKEN --claim-rooms a707fdee-792b-4e43-96b0-4a1a62771462 --claim-url https://app.netdata.cloud


echo "Starting nginx"
service nginx start &

echo "Starting HTTP server"
./jaydanhoward
