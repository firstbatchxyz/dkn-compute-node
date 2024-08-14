#!/bin/sh

docs() {
    echo "
    start.sh starts the compute node with given environment and parameters using docker-compose.
    Loads the .env file as base environment and creates a .env.compose file for final environment to run with docker-compose.
    Required environment variables in .env file; DKN_WALLET_SECRET_KEY
        
    Arguments:
        -h | --help: Displays this help message
        -m | --model: Indicates the model to be used within the compute node. Argument can be given multiple times for multiple models.
        -b | --background: Enables background mode for running the node (default: FOREGROUND)
        --dev: Sets the logging level to debug (default: false)
        --trace: Sets the logging level to trace (default: false)
        --docker-ollama: Indicates the Ollama docker image is being used (default: false)

    Example:
        ./start.sh -m=nous-hermes2theta-llama3-8b --model=phi3:medium --dev
    "
    exit 0
}

echo "************ DKN - Compute Node ************"

check_docker_compose() {
    # check "docker compose"
    if docker compose version >/dev/null 2>&1; then
        COMPOSE_COMMAND="docker compose"
    # check "docker-compose"
    elif docker-compose version >/dev/null 2>&1; then
        COMPOSE_COMMAND="docker-compose"
    else
        echo "docker compose is not installed on this machine. Its required to run the node."
        echo "Check https://docs.docker.com/compose/install/ for installation."
        exit 1
    fi
}
check_docker_compose

# if .env exists, load it first
ENV_FILE="./.env"
ENV_COMPOSE_FILE="./.env.compose"
if [ -f "$ENV_FILE" ]; then
  set -o allexport
  . "$ENV_FILE"
  set +o allexport
fi

# flag vars
START_MODE="FOREGROUND"
DOCKER_OLLAMA=false
RUST_LOG="none,dkn_compute=info" # default info logs

# script internal
COMPOSE_PROFILES=""
MODELS_LIST=""
LOCAL_OLLAMA_PID=""
DOCKER_HOST="http://host.docker.internal"

# handle command line arguments
while [ "$#" -gt 0 ]; do
    case $1 in
        -m=*|--model=*)
            # shift
            model="$(echo "${1#*=}" | tr '[:upper:]' '[:lower:]')"
            MODELS_LIST="$MODELS_LIST $model"
        ;;

        --docker-ollama)
            DOCKER_OLLAMA=true
        ;;

        --dev)
            RUST_LOG="none,dkn_compute=debug,ollama_workflows=info"
        ;;
        --trace)
            RUST_LOG="none,dkn_compute=trace"
        ;;
        -b|--background) START_MODE="BACKGROUND" ;;
        -h|--help) docs ;;
        *) echo "ERROR: Unknown parameter passed: $1"; exit 1 ;;
    esac
    shift
done

check_required_env_vars() {
    required_vars="
        DKN_WALLET_SECRET_KEY
        DKN_ADMIN_PUBLIC_KEY
    "
    for var in $required_vars; 
    do
        if [ -z "$(eval echo \$$var)" ]; 
        then
            echo "ERROR: $var environment variable is not set."
            exit 1
        fi
    done
}
check_required_env_vars

# helper function for writing given env-var pairs to .env.compose file as lines
write_to_env_file() {
  input_pairs="$*"

  # Write pairs to the file
  for pair in $input_pairs; do
    echo "$pair" >> "$ENV_COMPOSE_FILE"
  done
  echo "" >> "$ENV_COMPOSE_FILE"
}

# helper function for converting a list of env-var name to a list of env-var name:value pairs
as_pairs() {
    keys="$*"
    pairs=""
    for key in $keys; do
        value="$(eval echo \$$key)"
        if [ -z "$value" ]; then
            value=""
        fi

        pair="${key}=\"${value}\""
        pairs="$pairs $pair"
    done
    echo "$pairs"
}

echo "Setting up the environment..."

# this function handles all compute related environment, compute_envs is a list of "name=value" env-var pairs
compute_envs=""
handle_compute_env() {
    compute_env_vars="
        DKN_WALLET_SECRET_KEY
        DKN_ADMIN_PUBLIC_KEY
        OPENAI_API_KEY
        SERPER_API_KEY
        BROWSERLESS_TOKEN
        ANTHROPIC_API_KEY
        RUST_LOG
        DKN_MODELS
    "
    compute_envs=$(as_pairs $compute_env_vars)

    # handle DKN_MODELS
    if [ -n "$MODELS_LIST" ]; then
        # if model flag is given, pass it to env var
        DKN_MODELS=$(echo "$MODELS_LIST" | tr ' ' ',')
    fi

    # update envs
    compute_envs=$(as_pairs $compute_env_vars)
}
handle_compute_env

# this function handles all ollama related environment, ollama_envs is a list of "name=value" env-var pairs
ollama_envs=""
handle_ollama_env() {
    ollama_env_vars="
        OLLAMA_HOST
        OLLAMA_PORT
    "
    ollama_envs=$(as_pairs $ollama_env_vars)

    # if there is no ollama model given, do not add any ollama compose profile
    ollama_needed=false
    ollama_models="nous-hermes2theta-llama3-8b phi3:medium phi3:medium-128k phi3:3.8b llama3.1:latest"
    for m in $(echo "$DKN_MODELS" | tr ',' ' '); do
        case " $ollama_models " in
            *" $m "*) ollama_needed=true; break;;
        esac
    done
    if [ "$ollama_needed" = false ]; then
        echo "No Ollama model provided. Skipping the Ollama execution"
        return
    fi

    # check local ollama
    if [ "$DOCKER_OLLAMA" = false ]; then
        if command -v ollama >/dev/null 2>&1; then
            # prepare local ollama url
            OLLAMA_HOST="${OLLAMA_HOST:-http://localhost}"
            if [ -z "$OLLAMA_HOST" ] || [ "$OLLAMA_HOST" = "$DOCKER_HOST" ]; then
                OLLAMA_HOST="http://localhost"
            fi
            OLLAMA_PORT="${OLLAMA_PORT:-11434}"
            ollama_url=$OLLAMA_HOST:$OLLAMA_PORT

            # check whether ollama is serving or not
            check_ollama_server() {
                curl -s -o /dev/null -w "%{http_code}" ${ollama_url}
            }

            if [ "$(check_ollama_server)" -eq 200 ]; then
                echo "Local Ollama is already up and running, using it"
                OLLAMA_HOST=$DOCKER_HOST
                ollama_envs=$(as_pairs $ollama_env_vars)
                return
            else
                echo "Local Ollama is not live, running ollama serve"
                temp_ollama_host=$OLLAMA_HOST
                OLLAMA_HOST=$ollama_url # set temporarily OLLAMA_HOST env var for the ollama command
                # run ollama serve in background
                eval "ollama serve >/dev/null 2>&1 &"
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
                    DOCKER_OLLAMA=true
                else
                    LOCAL_OLLAMA_PID=$temp_pid
                    OLLAMA_HOST=$DOCKER_HOST
                    echo "Local Ollama server is up and running with PID $LOCAL_OLLAMA_PID"
                    ollama_envs=$(as_pairs $ollama_env_vars)
                    return
                fi
            fi
        else
            DOCKER_OLLAMA=true
            echo "Ollama is not installed on this machine, using the docker-compose service"
        fi
    fi

    # check for cuda gpu
    if command -v nvidia-smi >/dev/null 2>&1; then
        if nvidia-smi >/dev/null 2>&1; then
            echo "GPU type detected: CUDA"
            COMPOSE_PROFILES="$COMPOSE_PROFILES ollama-cuda"
            return
        fi
    fi

    # check for rocm gpu
    if command -v rocminfo >/dev/null 2>&1; then
        if rocminfo >/dev/null 2>&1; then
            echo "GPU type detected: ROCM"
            COMPOSE_PROFILES="$COMPOSE_PROFILES ollama-rocm"
            return
        fi
    fi

    # if there are no local ollama and gpu, use docker-compose with cpu profile
    echo "No GPU found, using ollama-cpu"
    COMPOSE_PROFILES="$COMPOSE_PROFILES ollama-cpu"
    OLLAMA_HOST=$DOCKER_HOST
    ollama_envs=$(as_pairs $ollama_env_vars)
}
handle_ollama_env

# env-var lists are ready, now write them to .env.compose
if [ -e "$ENV_COMPOSE_FILE" ]; then
    # if already exists, clean it first
    rm "$ENV_COMPOSE_FILE"
fi
write_to_env_file $compute_envs
write_to_env_file $ollama_envs

# update the image
echo ""
echo "Pulling the latest compute node image..."
DOCKER_CLI_HINTS=false docker pull firstbatch/dkn-compute-node:latest

# prepare compose profiles
COMPOSE_PROFILES=$(echo "$COMPOSE_PROFILES" | tr ' ' ',')
COMPOSE_PROFILES="COMPOSE_PROFILES=\"${COMPOSE_PROFILES}\""

# prepare compose commands
COMPOSE_UP="${COMPOSE_PROFILES} ${COMPOSE_COMMAND} up -d"
COMPOSE_DOWN="${COMPOSE_PROFILES} ${COMPOSE_COMMAND} down"

# run docker-compose up
echo ""
echo "Starting in ${START_MODE} mode..."
echo "${COMPOSE_UP}"
echo ""
eval "${COMPOSE_UP}"

compose_exit_code=$?

# handle docker-compose error
if [ $compose_exit_code -ne 0 ]; then
    echo ""
    echo "ERROR: docker-compose"
    exit $compose_exit_code
fi

echo "All good! Compute node is up and running."

# background/foreground mode
if [ "$START_MODE" = "FOREGROUND" ]; then
    echo ""
    echo "Use Control-C to exit"

    cleanup() {
        echo ""
        echo "Shutting down..."
        eval "${COMPOSE_DOWN}"
        rm "$ENV_COMPOSE_FILE"
        echo ""
        echo "bye"
        exit
    }
    # wait for Ctrl-C
    ( trap cleanup INT; read _ )
fi
