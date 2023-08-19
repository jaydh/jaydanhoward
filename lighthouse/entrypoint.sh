#!/bin/bash

set -m

MAX_RETRIES=10  # Maximum number of retries
RETRY_INTERVAL=5  # Time interval between retries in seconds

retry_count=0

while [ $retry_count -lt $MAX_RETRIES ]; do
    response_code=$(curl -s -o /dev/null -w "%{http_code}" $LEPTOS_SITE_ADDR/health_check)
    
    if [ "$response_code" -eq 200 ]; then
        echo "Health check successful (HTTP 200)."
        break
    else
        echo "Health check returned HTTP $response_code. Retrying in $RETRY_INTERVAL seconds..."
        sleep $RETRY_INTERVAL
        retry_count=$((retry_count + 1))
    fi
done

if [ $retry_count -eq $MAX_RETRIES ]; then
    echo "Health check did not succeed after $MAX_RETRIES retries."
    exit 1
fi

lighthouse --chrome-flags="--headless" http://$LEPTOS_SITE_ADDR/about
exit 0