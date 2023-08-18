#!/bin/bash

set -m

./jaydanhoward &

sleep 10s
lighthouse --chrome-flags="--headless" https://jaydanhoward-29npi.ondigitalocean.app
sleep 10m

fg %1
