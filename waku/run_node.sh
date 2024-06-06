#!/bin/sh

echo "I am a nwaku node"

if [ -z "${ETH_CLIENT_ADDRESS}" ]; then
    echo "Missing Eth client address, please refer to README.md for detailed instructions"
    exit 1
fi

MY_EXT_IP=$(wget -qO- https://api4.ipify.org)
DNS_WSS_CMD=

if [ -n "${DOMAIN}" ]; then

    LETSENCRYPT_PATH=/etc/letsencrypt/live/${DOMAIN}

    if ! [ -d "${LETSENCRYPT_PATH}" ]; then
        apk add --no-cache certbot

        certbot certonly\
            --non-interactive\
            --agree-tos\
            --no-eff-email\
            --no-redirect\
            --email admin@${DOMAIN}\
            -d ${DOMAIN}\
            --standalone
    fi

    if ! [ -e "${LETSENCRYPT_PATH}/privkey.pem" ]; then
        echo "The certificate does not exist"
        sleep 60
        exit 1
    fi

    WS_SUPPORT="--websocket-support=true"
    WSS_SUPPORT="--websocket-secure-support=true"
    WSS_KEY="--websocket-secure-key-path=${LETSENCRYPT_PATH}/privkey.pem"
    WSS_CERT="--websocket-secure-cert-path=${LETSENCRYPT_PATH}/cert.pem"
    DNS4_DOMAIN="--dns4-domain-name=${DOMAIN}"

    DNS_WSS_CMD="${WS_SUPPORT} ${WSS_SUPPORT} ${WSS_CERT} ${WSS_KEY} ${DNS4_DOMAIN}"
fi

if [ -n "${NODEKEY}" ]; then
    NODEKEY=--nodekey=${NODEKEY}
fi

RLN_RELAY_CRED_PATH=--rln-relay-cred-path=${RLN_RELAY_CRED_PATH:-/keystore/keystore.json}


if [ -n "${RLN_RELAY_CRED_PASSWORD}" ]; then
    RLN_RELAY_CRED_PASSWORD=--rln-relay-cred-password="${RLN_RELAY_CRED_PASSWORD}"
fi

STORE_RETENTION_POLICY=--store-message-retention-policy=size:1GB

if [ -n "${STORAGE_SIZE}" ]; then
    STORE_RETENTION_POLICY=--store-message-retention-policy=size:"${STORAGE_SIZE}"
fi

# we have disabled store for DKN, using --store=false

exec /usr/bin/wakunode\
  --relay=true\
  --filter=true\
  --lightpush=true\
  --keep-alive=true\
  --max-connections=150\
  --cluster-id=1\
  --discv5-discovery=true\
  --discv5-udp-port=9005\
  --discv5-enr-auto-update=True\
  --log-level=${LOG_LEVEL}\
  --tcp-port=30304\
  --metrics-server=True\
  --metrics-server-port=8003\
  --metrics-server-address=0.0.0.0\
  --rest=true\
  --rest-admin=true\
  --rest-address=0.0.0.0\
  --rest-port=8645\
  --rest-allow-origin="waku-org.github.io"\
  --nat=extip:"${MY_EXT_IP}"\
  --store=false\
  --store-message-db-url="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/postgres"\
  --rln-relay-eth-client-address="${ETH_CLIENT_ADDRESS}"\
  --rln-relay-tree-path="/etc/rln_tree"\
  ${RLN_RELAY_CRED_PATH}\
  ${RLN_RELAY_CRED_PASSWORD}\
  ${DNS_WSS_CMD}\
  ${NODEKEY}\
  ${STORE_RETENTION_POLICY}\
  ${EXTRA_ARGS}

