#!/bin/bash

# TODO: can remove this later

while true; do
  response=$(curl -s http://127.0.0.1:8645/health)
  echo -e "$(date +'%H:%M:%S')\t${response}"
  sleep 0.1
done
