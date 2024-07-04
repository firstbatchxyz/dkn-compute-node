#!/bin/sh

docs() {
    echo "
        start.sh starts the compute node with given environment and parameters using docker-compose.
        Loads the .env file as base environment and creates a .env.compose file for final environment to run with docker-compose.
        Required environment variables in .env file; RLN_RELAY_ETH_CLIENT_ADDRESS, ETH_TESTNET_KEY, RLN_RELAY_CRED_PASSWORD
        
        Description of command-line arguments:
            -m | --model: Indicates the model to be used within the compute node. Argument can be given multiple times for multiple models.

            --local-ollama=<true/false>: Indicates the local Ollama environment is being used (default: true)
            --waku-ext: Will assume Waku is running in another container already, and will not launch it. (default: false)

            --dev: Sets the logging level to debug (default: info)
            -b | --background: Enables background mode for running the node (default: FOREGROUND)
            -h | --help: Displays this help message

        Example usage:
            ./start.sh -m=nous-hermes2theta-llama3-8b --model=phi3:medium --local-ollama=false --dev
    "
    exit 0
}

echo "************ DKN - Compute Node ************"

# if .env exists, load it first
ENV_FILE=".env"
ENV_COMPOSE_FILE=".env.compose"
if [ -f "$ENV_FILE" ]; then
  set -o allexport
  source "$ENV_FILE"
  set +o allexport
fi

# flag vars
START_MODE="FOREGROUND"
LOCAL_OLLAMA=true
LOGS="info"
EXTERNAL_WAKU=false

# script internal
COMPOSE_PROFILES=()
MODELS_LIST=()
LOCAL_OLLAMA_PID=""
DOCKER_HOST="http://host.docker.internal"

# handle command line arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        -m=*|--model=*)
            # shift
            model="$(echo "${1#*=}" | tr '[:upper:]' '[:lower:]')"
            MODELS_LIST+=($model)
        ;;

        --local-ollama=*)
            LOCAL_OLLAMA="$(echo "${1#*=}" | tr '[:upper:]' '[:lower:]')"
        ;;

        --waku-ext)
            EXTERNAL_WAKU=true
        ;;

        --dev)
            DKN_LOG_LEVEL="none,dkn_compute=debug"
        ;;
        -b|--background) START_MODE="BACKGROUND" ;;
        -h|--help) docs ;;
        *) echo "ERROR: Unknown parameter passed: $1"; exit 1 ;;
    esac
    shift
done

check_required_env_vars() {
    local required_vars=(
        "RLN_RELAY_ETH_CLIENT_ADDRESS"
        "ETH_TESTNET_KEY"
        "RLN_RELAY_CRED_PASSWORD"
        "DKN_WALLET_SECRET_KEY"
        "DKN_ADMIN_PUBLIC_KEY"
    )
    for var in "${required_vars[@]}"; 
    do
        if [ -z "${!var}" ]; 
        then
            echo "ERROR: $var environment variable is not set."
            exit 1
        fi
    done
}
check_required_env_vars

# helper function for writing given env-var pairs to .env.compose file as lines
write_to_env_file() {
  local input_pairs=("$@")

  # Write pairs to the file
  for pair in "${input_pairs[@]}"; do
    echo "$pair" >> "$ENV_COMPOSE_FILE"
  done
  echo "" >> "$ENV_COMPOSE_FILE"
}

# helper function for converting a list of env-var name to a list of env-var name:value pairs
as_pairs() {
    local keys=("$@")
    pairs=()
    for i in "${!keys[@]}"; do
        key="${keys[i]}"
        value="$(eval echo \$$key)"
        if [ -z "$value" ]; then
            value=""
        fi

        pair="${key}=\"${value}\""
        pairs+=(${pair})
    done
    echo "${pairs[@]}"
}

echo "Handling the environment..."

# this function handles all compute related environment, compute_envs is a list of "name=value" env-var pairs
compute_envs=()
handle_compute_env() {
    compute_env_vars=(
        "DKN_WALLET_SECRET_KEY"
        "DKN_ADMIN_PUBLIC_KEY"
        "OPENAI_API_KEY"
        "SERPER_API_KEY"
        "BROWSERLESS_TOKEN"
        "ANTHROPIC_API_KEY"
        "DKN_LOG_LEVEL"
        "DKN_MODELS"
    )
    compute_envs=($(as_pairs "${compute_env_vars[@]}"))

    # handle DKN_MODELS
    if [ ${#MODELS_LIST[@]} -ne 0 ]; then
        # if model flag is given, pass it to env var
        DKN_MODELS=$(IFS=","; echo "${MODELS_LIST[*]}")
    fi

    # update envs
    compute_envs=($(as_pairs "${compute_env_vars[@]}"))
}
handle_compute_env

# this function handles all waku related environment, waku_envs is a list of "name=value" env-var pairs
waku_envs=()
handle_waku_env() {
    waku_env_vars=(
        "RLN_RELAY_ETH_CLIENT_ADDRESS"
        "ETH_TESTNET_KEY"
        "RLN_RELAY_CRED_PASSWORD"
        "WAKU_URL"
        "WAKU_EXTRA_ARGS"
        "WAKU_LOG_LEVEL"
    )
    # default value for waku url
    if [[ -z "$WAKU_URL" ]]; then
        WAKU_URL="http://host.docker.internal:8645"
    fi
    waku_envs=($(as_pairs "${waku_env_vars[@]}"))

    # add waku profile depending on EXTERNAL_WAKU flag
    if [ "$EXTERNAL_WAKU" == true ]; then
        echo "External waku is true, not running the waku"
        return
    else
        COMPOSE_PROFILES+=("waku")
    fi

    handle_waku_extra_args() {
        # get static waku peers
        # --staticnode
        WAKU_PEER_DISCOVERY_URL="" # TODO: url for getting a list of admin nodes in waku

        extra_args_list=()
        response=$(curl -s -X GET "$WAKU_PEER_DISCOVERY_URL" -d "param1=value1")
        parsed_response=$(echo "$response" | jq -r '.[]')
        if [[ -z "$parsed_response" ]]; then
            echo "No static peer set for waku"
        else
            waku_peers=""
            for peer in ${parsed_response[@]}; do
                waku_peers="${waku_peers}--staticnode=${peer} "
            done
            extra_args_list+=(${waku_peers})
        fi

        # TODO: additional waku-extra-args here
        extra_args=$(IFS=" "; echo "${extra_args_list[*]}")
        if [ -n "$extra_args" ]; then
            WAKU_EXTRA_ARGS="${WAKU_EXTRA_ARGS} ${extra_args}"
        fi
    }
    handle_waku_extra_args

    waku_envs=($(as_pairs "${waku_env_vars[@]}"))
}
handle_waku_env

# this function handles all ollama related environment, ollama_envs is a list of "name=value" env-var pairs
ollama_envs=()
handle_ollama_env() {
    ollama_env_vars=(
        "OLLAMA_HOST"
        "OLLAMA_PORT"
    )
    ollama_envs=($(as_pairs "${ollama_env_vars[@]}"))

    # if there is no ollama model given, do not add any ollama compose profile
    ollama_needed=false
    ollama_models=("nous-hermes2theta-llama3-8b" "phi3:medium" "phi3:medium-128k" "phi3:3.8b")
    IFS=',' read -r -a models <<< "$DKN_MODELS"
    for m in "${models[@]}"; do
        if [[ " ${ollama_models[@]} " =~ " ${m} " ]]; then
            ollama_needed=true
            break
        fi
    done
    if [ "$ollama_needed" = false ]; then
        echo "No Ollama model provided. Skipping the Ollama execution"
        return
    fi

    # check local ollama
    if [ "$LOCAL_OLLAMA" == true ]; then
        if command -v ollama &> /dev/null; then
            # prepare local ollama url
            OLLAMA_HOST="${OLLAMA_HOST:-http://localhost}"
            if [ -z "$OLLAMA_HOST" ] || [ "$OLLAMA_HOST" == "$DOCKER_HOST" ]; then
                OLLAMA_HOST="http://localhost"
            fi
            OLLAMA_PORT="${OLLAMA_PORT:-11434}"
            ollama_url=$OLLAMA_HOST:$OLLAMA_PORT

            # check whether ollama is serving or not
            check_ollama_server() {
                curl -s -o /dev/null -w "%{http_code}" ${ollama_url}
            }

            if [[ "$(check_ollama_server)" -eq 200 ]]; then
                echo "Local Ollama is already up and running, using it"
                OLLAMA_HOST=$DOCKER_HOST
                ollama_envs=($(as_pairs "${ollama_env_vars[@]}"))
                return
            else
                echo "Local Ollama is not live, running ollama serve"
                temp_ollama_host=$OLLAMA_HOST
                OLLAMA_HOST=$ollama_url # set temporarily OLLAMA_HOST env var for the ollama command
                # run ollama serve in background
                eval "ollama serve &>/dev/null &"
                temp_pid=$!

                MAX_RETRIES=5
                RETRY_COUNT=0
                # Loop until the server responds with HTTP 200 or the retry limit is reached
                until [ "$(check_ollama_server)" -eq 200 ] || [ "$RETRY_COUNT" -ge "$MAX_RETRIES" ]; do
                    echo "Waiting for the local ollama server to start... (Attempt $((RETRY_COUNT + 1))/$MAX_RETRIES)"
                    sleep 1
                    RETRY_COUNT=$((RETRY_COUNT + 1))
                done

                if [ "$RETRY_COUNT" -ge "$MAX_RETRIES" ]; then
                    echo "Local ollama server failed to start after $MAX_RETRIES attempts."
                    echo "Using docker-compose service"
                    LOCAL_OLLAMA=false
                else
                    LOCAL_OLLAMA_PID=$temp_pid
                    OLLAMA_HOST=$DOCKER_HOST
                    echo "Local Ollama server is up and running with PID $LOCAL_OLLAMA_PID"
                    ollama_envs=($(as_pairs "${ollama_env_vars[@]}"))
                    return
                fi
            fi
        else
            LOCAL_OLLAMA=false
            echo "Ollama is not installed on this machine, using the docker-compose service"
        fi
    fi

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

    # if there are no local ollama and gpu, use docker-compose with cpu profile
    echo "No GPU found, using ollama-cpu"
    COMPOSE_PROFILES+=("ollama-cpu")
    OLLAMA_HOST=$DOCKER_HOST
    ollama_envs=($(as_pairs "${ollama_env_vars[@]}"))
}
handle_ollama_env

# env-var lists are ready, now write them to .env.compose
if [ -e "$ENV_COMPOSE_FILE" ]; then
    # if already exists, clean it first
    rm "$ENV_COMPOSE_FILE"
fi
write_to_env_file "${waku_envs[@]}"
write_to_env_file "${compute_envs[@]}"
write_to_env_file "${ollama_envs[@]}"

# prepare compose profiles
COMPOSE_PROFILES=$(IFS=","; echo "${COMPOSE_PROFILES[*]}")
COMPOSE_PROFILES="COMPOSE_PROFILES=\"${COMPOSE_PROFILES}\""

# prepare compose commands
COMPOSE_COMMAND="docker-compose"
COMPOSE_UP="${COMPOSE_PROFILES} ${COMPOSE_COMMAND} up -d"
COMPOSE_DOWN="${COMPOSE_PROFILES} ${COMPOSE_COMMAND} down"

# run docker-compose up
echo "Starting in ${START_MODE} mode...\n"
echo "${COMPOSE_UP}\n"
eval "${COMPOSE_UP}"
compose_exit_code=$?

# handle docker-compose error
if [ $compose_exit_code -ne 0 ]; then
    echo "\nERROR: docker-compose"
    exit $compose_exit_code
fi

# background/foreground mode
if [ "$START_MODE" == "FOREGROUND" ]; then
    echo "\nUse Control-C to exit"

    cleanup() {
        echo "\nShutting down..."
        eval "${COMPOSE_DOWN}"
        rm "$ENV_COMPOSE_FILE"
        echo "\nbye"
        exit
    }
    # wait for Ctrl-C
    ( trap cleanup SIGINT ; read -r -d '' _ </dev/tty )
fi
