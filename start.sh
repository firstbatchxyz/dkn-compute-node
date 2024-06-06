#!/bin/sh

docs() {
    echo "
        Description of command-line arguments:
            -b, --background: Enables background mode for running the node (default: FOREGROUND)
            --search: Runs the node for the search tasks (default: false)
            --synthesis: Runs the node for the synthesis tasks (default: false)
            --local-ollama: Indicates the local Ollama environment is being used (default: false)
            --dev: Sets the logging level to debug (default: info)
            -h, --help: Displays this help message

        At least one of --search or --synthesis is required

        Example usage:
            ./start.sh --search --local-ollama --dev
    "
    exit 0
}

WAKU_PEER_DISCOVERY_URL="" # TODO: url for getting a list of admin nodes in waku

START_MODE="FOREGROUND"
COMPUTE_SEARCH=false
COMPUTE_SYNTHESIS=false
LOCAL_OLLAMA=false
LOGS="info"
COMPOSE_PROFILES=()
TASKS=()

echo "*** DKN - Compute Node ***"

# handle command line arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        -b|--background) START_MODE="BACKGROUND" ;;
        --search) COMPUTE_SEARCH=true ;;
        --synthesis) COMPUTE_SYNTHESIS=true ;;
        --local-ollama) LOCAL_OLLAMA=true ;;
        --dev) LOGS="debug" ;;
        -h|--help) docs ;;
        *) echo "ERROR: Unknown parameter passed: $1"; exit 1 ;;
    esac
    shift
done

echo "Handling the environment..."

ollama_profiles() {
    # check local ollama
    if [ "$LOCAL_OLLAMA" == true ]; then
        if command -v ollama &> /dev/null; then
            echo "Using local ollama"
            return
        else
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
    COMPOSE_PROFILES+=("ollama-cpu")
    return
}
ollama_profiles

# compute-node profiles
compute_options() {
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
                if [ "$ts" = "search" ] || [ "$ts" = "search-python" ]; then
                    TASKS+=("search")
                    COMPOSE_PROFILES+=("search-python") 
                elif [ "$ts" = "synthesis" ]; then
                    TASKS+=("synthesis")
                fi
            done

        else
            echo "ERROR: No task type has given, --synthesis and/or --search flags are required"
            exit 1
        fi
    fi
}
compute_options

WAKU_EXTRA_ARGS=()
handle_waku_args() {
    # --staticnode
    # get waku peers
    response=$(curl -s -X GET "$WAKU_PEER_DISCOVERY_URL" -d "param1=value1")
    parsed_response=$(echo "$response" | jq -r '.[]')
    if [[ -z "$parsed_response" ]]; then
        echo "No waku peer found"
    else
        waku_peers=""
        for peer in ${parsed_response[@]}; do
            waku_peers="${waku_peers}--staticnode=${peer} "
        done
        WAKU_EXTRA_ARGS+=(${waku_peers})
    fi

    # TODO: additional waku args here
}
handle_waku_args

# prepare docker-compose commands
COMPOSE_PROFILES=$(IFS=","; echo "${COMPOSE_PROFILES[*]}")
WAKU_EXTRA_ARGS=$(IFS=" "; echo "${WAKU_EXTRA_ARGS[*]}")
TASKS=$(IFS=,; echo "${TASKS[*]}")

compose_command="docker-compose"
compose_command="COMPOSE_PROFILES=\"${COMPOSE_PROFILES}\" ${compose_command}"
compose_command="WAKU_EXTRA_ARGS=\"${WAKU_EXTRA_ARGS}\" ${compose_command}"
compose_command="DKN_TASKS=\"${TASKS}\" ${compose_command}"
compose_command="RUST_LOGS=\"${LOGS}\" ${compose_command}"
compose_up="${compose_command} up -d"
compose_down="${compose_command} down"

# run docker-compose up
echo "\n"
echo "Starting in ${START_MODE} mode..."
eval "${compose_up}"
compose_exit_code=$?

# handle docker-compose error
if [ $compose_exit_code -ne 0 ]; then
    echo "\nERROR: docker-compose"
    exit $compose_exit_code
fi

# background/foreground mode
if [ "$START_MODE" == "FOREGROUND" ]; then
    echo "\nUse Command-C to exit"

    cleanup() {
        echo "\nShutting down..."
        eval "${compose_down}"
        echo "\nbye"
        exit
    }
    # wait for Ctrl-C
    ( trap cleanup SIGINT ; read -r -d '' _ </dev/tty )
fi
