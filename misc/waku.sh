#!/bin/bash

# get public ip for --nat option
# if you are behind a nat: --nat=any instead of public ip
PUBLIC_IP=$(dig TXT +short o-o.myaddr.l.google.com @ns1.google.com | awk -F'"' '{ print $2}')

docker run -i -t -p 60000:60000 -p 8645:8645 -p 9000:9000/udp harbor.status.im/wakuorg/nwaku:v0.26.0 \
  --dns-discovery=true \
  --dns-discovery-url=enrtree://AIRVQ5DDA4FFWLRBCHJWUWOO6X6S4ZTZ5B667LQ6AJU6PEYDLRD5O@sandbox.waku.nodes.status.im \
  --discv5-discovery=true \
  --nat=extip:$PUBLIC_IP \
  --rest=true --rest-address=0.0.0.0 \
  --name nwaku
