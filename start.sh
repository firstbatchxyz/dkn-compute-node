#!/bin/sh

# Description of command-line arguments:
#   --background: Enables background mode for running the node
# Example usage:
#   ./start.sh --background
# Required env-variables:
#   DKN_TASKS: comma seperated list of one or more items from the below list:
#       [synthesis,search,search-python]

WAKU_PEER_DISCOVERY_URL="" # TODO: url for getting a list of admin nodes in waku
BACKGROUND=false

echo "*** DKN - Compute Node ***"

while [[ "$#" -gt 0 ]]; do
    case $1 in
        --background) BACKGROUND=true ;;
        *) echo "Unknown parameter passed: $1"; exit 1 ;;
    esac
    shift
done

echo "Handling the environment..."

COMPOSE_PROFILES=()
# ollama profiles
check_gpu() {
    # check for cuda gpu
    if command -v nvidia-smi &> /dev/null; then
        if nvidia-smi &> /dev/null; then
            echo "GPU type detected: CUDA"
            COMPOSE_PROFILES+=("ollama-cuda")
            return
        fi
    fi

    # check for rocm gpu
    if command -v rocminfo &> /dev/null; then
        if rocminfo &> /dev/null; then
            echo "GPU type detected: ROCM"
            COMPOSE_PROFILES+=("ollama-rocm")
            return
        fi
    fi

    echo "No compatible GPU detected, running on CPU"
    COMPOSE_PROFILES+=("ollama-cpu")
    return
}
check_gpu

# compute-node profiles
compute_node_profiles() {
    # Check if DKN_TASKS is set
    if [ -z "$DKN_TASKS" ]; then
        echo "DKN_TASKS environment variable is not set"
        return 1
    fi

    IFS=',' read -ra modes <<< "$DKN_TASKS"
    for mode in "${modes[@]}"; do
        COMPOSE_PROFILES+=($mode)
    done

}
compute_node_profiles

WAKU_EXTRA_ARGS=()
handle_waku_args() {
    # --staticnode
    # get waku peers
    response=$(curl -s -X GET "$WAKU_PEER_DISCOVERY_URL" -d "param1=value1")
    parsed_response=$(echo "$response" | jq -r '.[]')
    if [[ -z "$parsed_response" ]]; then
        echo "No waku peer found"
    else
        waku_peers="--staticnode="
        for peer in $parsed_response; do
            waku_peers="${waku_peers},${peer}"
        done
        WAKU_EXTRA_ARGS+=(${waku_peers})
    fi

    # TODO: additional waku args here
}
handle_waku_args

# prepare docker-compose commands
compose_command="docker-compose"
COMPOSE_PROFILES=$(IFS=,; echo "${COMPOSE_PROFILES[*]}")
WAKU_EXTRA_ARGS=$(IFS=,; echo "${WAKU_EXTRA_ARGS[*]}")
compose_command="WAKU_EXTRA_ARGS=\"${WAKU_EXTRA_ARGS}\" ${compose_command}"
compose_command="COMPOSE_PROFILES=\"${COMPOSE_PROFILES}\" ${compose_command}"
compose_up="${compose_command} up -d"
compose_down="${compose_command} down"

# run docker-compose
echo "\n"
if [ "$BACKGROUND" == true ]; then
    echo "Starting in background mode..."
    eval "${compose_up} > /dev/null 2>&1"
else
    echo "Starting in attached mode..."
    eval "${compose_up} > /dev/null 2>&1"
    echo "Use Command-C to exit"
    cleanup() {
        echo "\nShutting down..."
        eval "${compose_down} > /dev/null 2>&1"
        echo "bye"
        exit
    }
    # wait for Ctrl-C
    ( trap cleanup SIGINT ; read -r -d '' _ </dev/tty )
fi
