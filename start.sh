#!/bin/sh

docs() {
    echo "
        Description of command-line arguments:
            --synthesis: Runs the node for the synthesis tasks. Can be set as DKN_TASKS="synthesis" env-var along. (default: false, required for search tasks)
            --search: Runs the node for the search tasks. Can be set as DKN_TASKS="search" env-var (default: false, required for synthesis tasks)

            --synthesis-model-provider=<arg>: Indicates the model provider for synthesis tasks, ollama or openai. Can be set as DKN_SYNTHESIS_MODEL_PROVIDER env-var (required on synthesis tasks)
            --search-model-provider=<arg>: Indicates the model provider for search tasks, ollama or openai. Can be set as AGENT_MODEL_PROVIDER env-var (required on search tasks)

            --synthesis-model: Indicates the model for synthesis tasks, model needs to be compatible with the given provider. Can be set as DKN_SYNTHESIS_MODEL_NAME env-var (required on synthesis tasks) 
            --search-model: Indicates the model for search tasks, model needs to be compatible with the given provider. Can be set as AGENT_MODEL_NAME env-var (required on search tasks) 

            --local-ollama=<true/false>: Indicates the local Ollama environment is being used (default: true)

            --dev: Sets the logging level to debug (default: info)
            -b, --background: Enables background mode for running the node (default: FOREGROUND)
            -h, --help: Displays this help message

        At least one of --search or --synthesis is required

        Example usage:
            ./start.sh --search --synthesis --local-ollama=false  --dev
    "
    exit 0
}

echo "*** DKN - Compute Node ***"

# if .env exists, load it first
if [ -f .env ]; then
  set -o allexport
  source .env
  set +o allexport
fi

START_MODE="FOREGROUND"
COMPUTE_SEARCH=false
COMPUTE_SYNTHESIS=false
LOCAL_OLLAMA=true
LOGS="info"
COMPOSE_PROFILES=()
TASKS=()
LOCAL_OLLAMA_PID=""

# handle command line arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        -b|--background) START_MODE="BACKGROUND" ;;
        
        --search) COMPUTE_SEARCH=true ;;
        --synthesis) COMPUTE_SYNTHESIS=true ;;

        --synthesis-model-provider=*)
        DKN_SYNTHESIS_MODEL_PROVIDER="$(echo "${1#*=}" | tr '[:upper:]' '[:lower:]')"
        ;;
        --search-model-provider=*)
        AGENT_MODEL_PROVIDER="$(echo "${1#*=}" | tr '[:upper:]' '[:lower:]')"
        ;;

        --synthesis-model=*)
        DKN_SYNTHESIS_MODEL_NAME="$(echo "${1#*=}" | tr '[:upper:]' '[:lower:]')"
        ;;
        --search-model=*)
        AGENT_MODEL_NAME="$(echo "${1#*=}" | tr '[:upper:]' '[:lower:]')"
        ;;

        --local-ollama=*)
        LOCAL_OLLAMA="$(echo "${1#*=}" | tr '[:upper:]' '[:lower:]')"
        ;;
        --dev) LOGS="debug" ;;
        -h|--help) docs ;;
        *) echo "ERROR: Unknown parameter passed: $1"; exit 1 ;;
    esac
    shift
done

echo "Handling the environment..."

check_required_env_vars() {
    local required_vars=("ETH_CLIENT_ADDRESS" "ETH_TESTNET_KEY" "RLN_RELAY_CRED_PASSWORD" "WAKU_URL")
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

task_options() {
    if [ "$COMPUTE_SEARCH" = true ]; then
        TASKS+=("search")
        COMPOSE_PROFILES+=("search-python") # start with search-agent profile for search dependencies
    fi
    if [ "$COMPUTE_SYNTHESIS" = true ]; then
        TASKS+=("synthesis")
    fi
    if [ "$COMPUTE_SEARCH" = false ] && [ "$COMPUTE_SYNTHESIS" = false ]; then
        # if no task type argument has given, check DKN_TASKS env var
        if [ -n "$DKN_TASKS" ]; then
            # split, iterate and validate given tasks in env var 
            IFS=',' read -ra tsks <<< "$DKN_TASKS"
            for ts in "${tsks[@]}"; do
                ts="$(echo "${ts#*=}" | tr '[:upper:]' '[:lower:]')" # make all lowercase
                if [ "$ts" = "search" ] || [ "$ts" = "search-python" ]; then
                    TASKS+=("search")
                    COMPOSE_PROFILES+=("search-python")
                    COMPUTE_SEARCH=true
                elif [ "$ts" = "synthesis" ]; then
                    TASKS+=("synthesis")
                    COMPUTE_SYNTHESIS=true
                fi
            done

        else
            echo "ERROR: No task type has given, --synthesis and/or --search flags are required"
            exit 1
        fi
    fi
}
task_options

check_model_providers() {
    if [ "$COMPUTE_SEARCH" = true ]; then
        if [ -z "$AGENT_MODEL_PROVIDER" ]; then
            echo "ERROR: Search model provider is required on search tasks. Example usage; --search-model-provider=ollama"
            exit 1
        fi

    fi
    if [ "$COMPUTE_SYNTHESIS" = true ]; then
        if [ -z "$DKN_SYNTHESIS_MODEL_PROVIDER" ]; then
            echo "ERROR: Synthesis model provider is required on synthesis tasks. Example usage; --synthesis-model-provider=ollama"
            exit 1
        fi
    fi
}
check_model_providers

ollama_profiles() {
    # if there is no task using ollama, do not add any ollama profile
    DKN_SYNTHESIS_MODEL_PROVIDER="$(echo "${DKN_SYNTHESIS_MODEL_PROVIDER#*=}" | tr '[:upper:]' '[:lower:]')"
    AGENT_MODEL_PROVIDER="$(echo "${AGENT_MODEL_PROVIDER#*=}" | tr '[:upper:]' '[:lower:]')"
    ollama_needed=false
    if [ "$COMPUTE_SYNTHESIS" = true ] && [ "$DKN_SYNTHESIS_MODEL_PROVIDER" == "ollama" ]; then
        ollama_needed=true
    fi
    if [ "$COMPUTE_SEARCH" = true ] && [ "$AGENT_MODEL_PROVIDER" == "ollama" ]; then
        ollama_needed=true
    fi
    if [ "$ollama_needed" = false ]; then
        return
    fi

    # check local ollama
    if [ "$LOCAL_OLLAMA" == true ]; then
        if command -v ollama &> /dev/null; then
            # prepare local ollama url
            OLLAMA_HOST="${OLLAMA_HOST:-http://localhost}"
            OLLAMA_PORT="${OLLAMA_PORT:-11434}"
            ollama_url=$OLLAMA_HOST:$OLLAMA_PORT

            # check whether ollama is serving or not
            check_ollama_server() {
                curl -s -o /dev/null -w "%{http_code}" ${ollama_url}
            }

            if [[ "$(check_ollama_server)" -eq 200 ]]; then
                echo "Local Ollama is already up and running, using it"
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
                    OLLAMA_HOST=$temp_ollama_host
                    echo "Local Ollama server is up and running with PID $LOCAL_OLLAMA_PID"
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
    return
}
ollama_profiles

wake_arg_list=()
# WAKU_EXTRA_ARGS=()
WAKU_PEER_DISCOVERY_URL="" # TODO: url for getting a list of admin nodes in waku
handle_waku_args() {
    # --staticnode
    # get waku peers
    response=$(curl -s -X GET "$WAKU_PEER_DISCOVERY_URL" -d "param1=value1")
    parsed_response=$(echo "$response" | jq -r '.[]')
    if [[ -z "$parsed_response" ]]; then
        echo "No static peer set for waku"
    else
        waku_peers=""
        for peer in ${parsed_response[@]}; do
            waku_peers="${waku_peers}--staticnode=${peer} "
        done
        wake_arg_list+=(${waku_peers})
    fi
    wake_arg_list=$(IFS=" "; echo "${wake_arg_list[*]}")
    if [ -n "$wake_arg_list" ]; then
        WAKU_EXTRA_ARGS="${WAKU_EXTRA_ARGS} ${wake_arg_list}"
    fi

    # TODO: additional waku args here
}
handle_waku_args

# prepare env-vars
ENVVARS=""
handle_env_vars(){
    COMPOSE_PROFILES=$(IFS=","; echo "${COMPOSE_PROFILES[*]}")
    ENVVARS="COMPOSE_PROFILES=\"${COMPOSE_PROFILES}\" ${ENVVARS}"

    ENVVARS="WAKU_EXTRA_ARGS=\"${WAKU_EXTRA_ARGS}\" ${ENVVARS}"

    TASKS=$(IFS=,; echo "${TASKS[*]}")
    ENVVARS="DKN_TASKS=${TASKS} ${ENVVARS}"

    # set non-empty configs as env vars
    if [ -n "$DKN_SYNTHESIS_MODEL_PROVIDER" ]; then
        ENVVARS="DKN_SYNTHESIS_MODEL_PROVIDER=\"${DKN_SYNTHESIS_MODEL_PROVIDER}\" ${ENVVARS}"
    fi
    if [ -n "$AGENT_MODEL_PROVIDER" ]; then
        ENVVARS="AGENT_MODEL_PROVIDER=\"${AGENT_MODEL_PROVIDER}\" ${ENVVARS}"
    fi
    if [ -n "$DKN_SYNTHESIS_MODEL_NAME" ]; then
        ENVVARS="DKN_SYNTHESIS_MODEL_NAME=\"${DKN_SYNTHESIS_MODEL_NAME}\" ${ENVVARS}"
    fi
    if [ -n "$AGENT_MODEL_NAME" ]; then
        ENVVARS="AGENT_MODEL_NAME=\"${AGENT_MODEL_NAME}\" ${ENVVARS}"
    fi
    ENVVARS="RUST_LOG=\"${LOGS}\" ${ENVVARS}"
    # if [ "$LOCAL_OLLAMA" = true ]; then
    #     ENVVARS="OLLAMA_HOST=\"http://host.docker.internal\" ${ENVVARS}"
    # fi
}
handle_env_vars

# prepare compose commands
COMPOSE_COMMAND="docker-compose"
COMPOSE_UP="${ENVVARS} ${COMPOSE_COMMAND} up -d"
COMPOSE_DOWN="${ENVVARS} ${COMPOSE_COMMAND} down"

# run docker-compose up
echo "\n"
echo "Starting in ${START_MODE} mode..."
echo "${COMPOSE_UP}"
# eval "${COMPOSE_UP}"
compose_exit_code=$?

# handle docker-compose error
if [ $compose_exit_code -ne 0 ]; then
    echo "\nERROR: docker-compose"
    exit $compose_exit_code
fi

# # background/foreground mode
# if [ "$START_MODE" == "FOREGROUND" ]; then
#     echo "\nUse Control-C to exit"

#     cleanup() {
#         echo "\nShutting down..."
#         eval "${COMPOSE_DOWN}"
#         echo "\nbye"
#         exit
#     }
#     # wait for Ctrl-C
#     ( trap cleanup SIGINT ; read -r -d '' _ </dev/tty )
# fi
