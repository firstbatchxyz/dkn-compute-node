#!/bin/sh

if test -f $(pwd)/keystore/keystore.json; then
  echo "Keystore already exists. Use it instead of creating a new one."
  echo "Exiting"
  exit 1
fi

if test -f .env; then
  echo "Using .env file"  
  . $(pwd)/../.env
fi

docker run -v $(pwd)/keystore:/keystore/:Z harbor.status.im/wakuorg/nwaku:v0.25.0 generateRlnKeystore \
--rln-relay-eth-client-address=${ETH_CLIENT_ADDRESS} \
--rln-relay-eth-private-key=${ETH_TESTNET_KEY} \
--rln-relay-eth-contract-address=0xF471d71E9b1455bBF4b85d475afb9BB0954A29c4 \
--rln-relay-cred-path=/keystore/keystore.json \
--rln-relay-cred-password="${RLN_RELAY_CRED_PASSWORD}" \
--execute
