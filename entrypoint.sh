#!/bin/bash

set -m

service nginx start

./jaydanhoward &

fg %1
