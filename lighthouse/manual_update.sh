#!/bin/bash

sudo docker build -t lighthouse .
# Prompt the user for the secret token
read -p "Enter your LIGHTHOUSE_UPDATE_TOKEN: " secret_token

# Run the Docker container with the secret token as an environment variable
sudo docker run -e "LIGHTHOUSE_UPDATE_TOKEN=$secret_token" -t lighthouse
