#!/bin/bash

set -m

./jaydanhoward &

sleep 30s
(cd /home/chrome && runuser -l  chrome -c 'lighthouse --chrome-flags="--headless" https://jaydanhoward-29npi.ondigitalocean.app') 
cp /home/chrome/jaydanhoward*.report.html /app/site/lighthouse.html
sleep 10m

fg %1
