package main

import (
	"bufio"
	"encoding/hex"
	"flag"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"time"

	"github.com/joho/godotenv"
)

func isCommandAvailable(command string) bool {
	// LookPath searches for an executable named command in the directories
	// named by the PATH environment variable.
	_, err := exec.LookPath(command)
	return err == nil
}

func runCommand(printToStdout, wait bool, envs []string, command string, args ...string) (int, error) {
	cmd := exec.Command(command, args...)

	// Set the environment variable
	cmd.Env = append(os.Environ(), envs...)

	// set working dir
	cmd.Dir = WORKING_DIR

	if printToStdout {
		// Connect stdout and stderr to the terminal
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
	} else {
		// Capture output if not printing to stdout
		cmd.Stdout = nil
		cmd.Stderr = nil
	}

	// Start the command
	err := cmd.Start()
	if err != nil {
		return 0, fmt.Errorf("failed to start command: %w", err)
	}

	// Get the PID
	pid := cmd.Process.Pid

	// Wait for the command to finish
	if wait {
		err = cmd.Wait()
		if err != nil {
			return pid, fmt.Errorf("command finished with error: %w", err)
		}
	}
	return pid, nil
}

func checkDockerComposeCommand() (string, []string, []string) {
	// check docker compose
	if _, err := runCommand(false, true, nil, "docker", "compose", "version"); err == nil {
		return "docker", []string{"compose", "up", "-d"}, []string{"compose", "down"}
	}

	// check docker-compose
	if _, err := runCommand(false, true, nil, "docker-compose", "version"); err == nil {
		return "docker-compose", []string{"up", "-d"}, []string{"down"}
	}

	// both not found, exit
	fmt.Println("docker compose is not installed on this machine. It's required to run the node.")
	fmt.Println("Check https://docs.docker.com/compose/install/ for installation.")
	exitWithDelay(1)
	return "", nil, nil
}

func isDockerUp() bool {
	_, err := runCommand(false, true, nil, "docker", "info")
	return err == nil
}

func checkRequiredEnvVars(envvars map[string]string) {
	absent_env := false
	if envvars["DKN_WALLET_SECRET_KEY"] == "" {
		fmt.Println("DKN_WALLET_SECRET_KEY env-var is not set, getting it interactively")
		skey, err := getDknSecretKey()
		if err != nil {
			fmt.Printf("Error during user input: %s\n", err)
			exitWithDelay(1)
		}
		envvars["DKN_WALLET_SECRET_KEY"] = skey
		absent_env = true
	}

	if envvars["DKN_ADMIN_PUBLIC_KEY"] == "" {
		envvars["DKN_ADMIN_PUBLIC_KEY"] = DKN_ADMIN_PUBLIC_KEY
		absent_env = true
	}

	if absent_env {
		// dump the .env file for future usages
		fmt.Printf("Dumping the given env-vars to .env\n\n")
		godotenv.Write(envvars, filepath.Join(WORKING_DIR, ".env"))
	}
}

func setWorkingDir() {
	ex, err := os.Executable()
	if err != nil {
		fmt.Printf("Error during getting the working directory %s\n", err)
		exitWithDelay(1)
	}
	WORKING_DIR = filepath.Dir(ex)
}

func getDknSecretKey() (string, error) {
	reader := bufio.NewReader(os.Stdin)
	// get DKN_WALLET_SECRET_KEY
	fmt.Print("Please enter your DKN Wallet Secret Key (32-bytes hex encoded): ")
	skey, err := reader.ReadString('\n')
	if err != nil {
		return "", fmt.Errorf("couldn't get DKN Wallet Secret Key")
	}
	skey = strings.TrimSpace(skey)
	skey = strings.Split(skey, "\n")[0]
	skey = strings.TrimPrefix(skey, "0x")
	// decode the hex string into bytes
	decoded_skey, err := hex.DecodeString(skey)
	if err != nil {
		return "", fmt.Errorf("DKN Wallet Secret Key should be 32-bytes hex encoded")
	}
	// ensure the decoded bytes are exactly 32 bytes
	if len(decoded_skey) != 32 {
		return "", fmt.Errorf("DKN Wallet Secret Key should be 32 bytes long")
	}
	return skey, nil
}

func mapToList(m map[string]string) []string {
	var list []string
	for key, value := range m {
		list = append(list, fmt.Sprintf("%s=%s", key, value))
	}
	return list
}

func isOllamaServing(host, port string) bool {
	client := http.Client{
		Timeout: 2 * time.Second,
	}

	resp, err := client.Get(fmt.Sprintf("%s:%s", host, port))
	if err != nil {
		return false
	}
	defer resp.Body.Close()

	return resp.StatusCode == http.StatusOK
}

func runOllamaServe(host, port string) (int, error) {
	var cmd *exec.Cmd

	ollama_env := fmt.Sprintf("OLLAMA_HOST=%s:%s", host, port)
	pid, err := runCommand(false, false, []string{ollama_env}, "ollama", "serve")
	if err != nil {
		return 0, fmt.Errorf("failed during running ollama serve: %w", err)
	}

	for retryCount := 0; retryCount < OLLAMA_MAX_RETRIES; retryCount++ {
		if isOllamaServing(host, port) {
			return pid, nil
		}
		fmt.Printf("Waiting for the local Ollama server to start... (Attempt %d/%d)\n", retryCount+1, OLLAMA_MAX_RETRIES)
		time.Sleep(2 * time.Second)
	}

	cmd.Process.Kill()
	return pid, fmt.Errorf("ollama failed to start after %d retries", OLLAMA_MAX_RETRIES)
}

func handleOllamaEnv(ollamaHost, ollamaPort string, dockerOllama bool) (string, string, string, string) {
	// local ollama
	if !dockerOllama {
		if isCommandAvailable("ollama") {
			// host machine has ollama installed
			// we first going to check whether its serving or not
			// if not script runs ollama serve command manually and stores its pid

			// prepare local ollama url
			if ollamaHost == "" || ollamaHost == DOCKER_HOST {
				// we have to check Ollama at host, but if the given host is
				// host.docker.internal we still have to check the localhost
				ollamaHost = LOCAL_HOST
			}
			if ollamaPort == "" {
				ollamaPort = strconv.Itoa(DEFAULT_OLLAMA_PORT)
			}

			// check is it already serving
			if isOllamaServing(ollamaHost, ollamaPort) {
				fmt.Printf("Local Ollama is already up at %s:%s and running, using it\n", ollamaHost, ollamaPort)
			} else {
				// ollama is not live, so we launch it ourselves
				fmt.Println("Local Ollama is not live, running ollama serve")
				ollama_pid, err := runOllamaServe(ollamaHost, ollamaPort)
				if err != nil {
					// ollama failed to start, exit
					fmt.Println(err)
					fmt.Println("You can use the --docker-ollama flag to use the Docker Ollama image instead.")
					exitWithDelay(1)
				} else {
					fmt.Printf("Local Ollama server is up at %s:%s and running with PID %d\n", ollamaHost, ollamaPort, ollama_pid)
				}
			}

			// to use the local Ollama, we need to configure the network depending on the Host
			// Windows and Mac should work with host.docker.internal alright,
			// but Linux requires `host` network mode with `localhost` as the Host URL
			if runtime.GOOS == "darwin" {
				ollamaHost = DOCKER_HOST
			} else if runtime.GOOS == "windows" {
				ollamaHost = DOCKER_HOST
			} else if runtime.GOOS == "linux" {
				ollamaHost = LOCAL_HOST
			}
		} else {
			// although --docker-ollama was not passed, we checked and couldnt find Ollama
			// so we will use Docker anyways
			fmt.Println("Ollama is not installed on this machine, will use Docker Ollama service.")
			dockerOllama = true
		}
	}

	composeProfile := ""
	if dockerOllama {
		// using docker-ollama, check profiles
		if isCommandAvailable("nvidia-smi") {
			composeProfile = "ollama-cuda"
			fmt.Println("GPU type detected: CUDA")
		} else if isCommandAvailable("rocminfo") {
			fmt.Println("GPU type detected: ROCM")
			composeProfile = "ollama-rocm"
		} else {
			fmt.Println("No GPU found, using ollama-cpu")
			composeProfile = "ollama-cpu"
		}

		// since docker-ollama is using, set docker.internal for the Ollama host
		ollamaHost = DOCKER_HOST
		ollamaPort = strconv.Itoa(DEFAULT_OLLAMA_PORT)
	}

	// depending on the OS, use host or bridge network modes
	// https://docs.docker.com/engine/network/#drivers
	dockerNetworkMode := ""
	if runtime.GOOS == "darwin" {
		dockerNetworkMode = "bridge"
	} else if runtime.GOOS == "windows"  {
		dockerNetworkMode = "bridge"
	} else if runtime.GOOS == "linux" {
		dockerNetworkMode = "host"
	} 

	return ollamaHost, ollamaPort, dockerNetworkMode, composeProfile
}

func formatMapKeys(m map[string]bool) string {
	var keys []string
	for key := range m {
		keys = append(keys, key)
	}
	return "[" + strings.Join(keys, ", ") + "]"
}

func pickModels() string {
	reader := bufio.NewReader(os.Stdin)
	fmt.Print("Please pick the model you want to run:\n\n")
	fmt.Printf("ID\tProvider\tName\n")
	for id, model := range OPENAI_MODELS {
		fmt.Printf("%d\tOpenAI\t%s\n", id+1, model)
	}
	for id, model := range OLLAMA_MODELS {
		fmt.Printf("%d\tOllama\t%s\n", len(OPENAI_MODELS)+id+1, model)
	}
	fmt.Printf("Enter the model ids (comma seperated, e.g: 1,2,4): ")
	models, err := reader.ReadString('\n')
	if err != nil {
		return ""
	}
	models = strings.TrimSpace(models)
	models = strings.Split(models, "\n")[0]
	models = strings.ReplaceAll(models, " ", "")
	models_list := strings.Split(models, ",")
	picked_models_map := make(map[int]bool, 0)
	picked_models_str := ""
	invalid_selections := make(map[string]bool, 0)
	for _, i := range models_list {
		// if selection is already in invalids list, continue
		if invalid_selections[i] || i == "" {
			continue
		}

		id, err := strconv.Atoi(i)
		if err != nil {
			// not integer, invalid
			invalid_selections[i] = true
			continue
		}
		if id > 0 && id <= len(OPENAI_MODELS) {
			// openai model picked
			if !picked_models_map[id] {
				// if not already picked, add it to bin
				picked_models_map[id] = true
				picked_models_str = fmt.Sprintf("%s,%s", picked_models_str, OPENAI_MODELS[id-1])
			}
		} else if id > len(OPENAI_MODELS) && id <= len(OLLAMA_MODELS)+len(OPENAI_MODELS) {
			// ollama model picked
			if !picked_models_map[id] {
				// if not already picked, add it to bin
				picked_models_map[id] = true
				picked_models_str = fmt.Sprintf("%s,%s", picked_models_str, OLLAMA_MODELS[id-len(OPENAI_MODELS)-1])
			}
		} else {
			// out of index, invalid
			invalid_selections[i] = true
			continue
		}
	}
	if len(invalid_selections) != 0 {
		fmt.Printf("Skipping the invalid selections: %s \n", formatMapKeys(invalid_selections))
	}
	fmt.Printf("\n")
	return picked_models_str
}

func getUserInput(message string, trim bool) string {
	reader := bufio.NewReader(os.Stdin)
	fmt.Printf("%s: ", message)
	answer, err := reader.ReadString('\n')
	if err != nil {
		return ""
	}
	answer = strings.TrimSpace(answer)
	answer = strings.Split(answer, "\n")[0]
	if trim {
		answer = strings.ReplaceAll(answer, " ", "")
	}
	return answer
}

func exitWithDelay(code int) {
	fmt.Println("Terminating in 5 seconds...")
	time.Sleep(5 * time.Second)
	os.Exit(code)
}

type ModelList []string

func (models *ModelList) String() string {
	str := ""
	for _, m := range *models {
		str = fmt.Sprintf("%s, %s", str, m)
	}
	return str
}

func (models *ModelList) Set(value string) error {
	*models = append(*models, value)
	return nil
}

var (
	OLLAMA_MODELS       = []string{
		"adrienbrault/nous-hermes2theta-llama3-8b:q8_0",
		"phi3:14b-medium-4k-instruct-q4_1",
		"phi3:14b-medium-128k-instruct-q4_1",
		"phi3:3.8b",
		"phi3.5:3.8b",
		"phi3.5:3.8b-mini-instruct-fp16",
		"llama3.1:latest",
		"llama3.1:8b-instruct-q8_0",
	}
	OPENAI_MODELS       = []string{"gpt-3.5-turbo","gpt-4-turbo","gpt-4o","gpt-4o-mini"}
	DEFAULT_OLLAMA_PORT = 11434
	DOCKER_HOST         = "http://host.docker.internal"
	LOCAL_HOST          = "http://localhost"
	OLLAMA_MAX_RETRIES  = 5

	// Default admin public key, it will be used unless --dkn-admin-public-key is given
	DKN_ADMIN_PUBLIC_KEY = "0208ef5e65a9c656a6f92fb2c770d5d5e2ecffe02a6aade19207f75110be6ae658"

	WORKING_DIR = ""
)

func main() {
	fmt.Println("************ DKN - Compute Node ************")

	help := flag.Bool("h", false, "Displays this help message")
	flag.BoolVar(help, "help", false, "Displays this help message")
	var models ModelList
	flag.Var(&models, "m", "Indicates the model to be used within the compute node. Can be used multiple times for multiple models.")
	flag.Var(&models, "model", "Indicates the model to be used within the compute node. Can be used multiple times for multiple models.")
	background := flag.Bool("b", false, "Enables background mode for running the node (default: FOREGROUND)")
	flag.BoolVar(background, "background", false, "Enables background mode for running the node (default: FOREGROUND)")
	dev := flag.Bool("dev", false, "Sets the logging level to debug (default: false)")
	trace := flag.Bool("trace", false, "Sets the logging level to trace (default: false)")
	dockerOllama := flag.Bool("docker-ollama", false, "Indicates the Ollama docker image is being used (default: false)")
	dkn_admin_pkey_flag := flag.String("dkn-admin-public-key", DKN_ADMIN_PUBLIC_KEY, "DKN Admin Node Public Key, usually dont need this since it's given by default")
	pick_model := flag.Bool("pick-models", false, "Pick the models using cli, supprases the -m flags (default: false)")

	flag.Parse()
	// override DKN_ADMIN_PUBLIC_KEY if flag is a different value
	DKN_ADMIN_PUBLIC_KEY = *dkn_admin_pkey_flag

	// Display help and exit if -h or --help is provided
	if *help {
		flag.Usage()
		os.Exit(0)
	}

	fmt.Printf("Setting up the environment...\n\n")

	// get the current working directory and set it to global WORKING_DIR
	setWorkingDir()

	// Check Docker Compose
	composeCommand, composeUpArgs, composeDownArgs := checkDockerComposeCommand()
	if !isDockerUp() {
		fmt.Println("ERROR: Docker is not up")
		exitWithDelay(1)
	}

	// first load .env file if exists
	envvars, err := godotenv.Read(filepath.Join(WORKING_DIR, ".env"))
	if err != nil {
		// if couldnt load the .env, use .env.example
		envvars, err = godotenv.Read(filepath.Join(WORKING_DIR, ".env.example"))
		if err != nil {
			fmt.Println("Couldn't locate both .env and .env.example")
			exitWithDelay(1)
		}
	}

	checkRequiredEnvVars(envvars)

	// if -m flag is given, set them as DKN_MODELS
	if len(models) != 0 {
		envvars["DKN_MODELS"] = strings.Join(models, ",")
	}

	// if DKN_MODELS are still empty, pick model interactively
	if envvars["DKN_MODELS"] == "" || *pick_model {
		pickedModels := pickModels()
		if pickedModels == "" {
			fmt.Println("No valid model picked")
			exitWithDelay(1)
		}
		envvars["DKN_MODELS"] = pickedModels
	}

	// check openai api key
	for _, model := range strings.Split(envvars["DKN_MODELS"], ",") {
		for _, openai_model := range OPENAI_MODELS {
			if model == openai_model {
				if envvars["OPENAI_API_KEY"] == "" {
					apikey := getUserInput("Enter your OpenAI API Key", true)
					if apikey == "" {
						fmt.Printf("Invalid input, please place your OPENAI_API_KEY to .env file\n")
						exitWithDelay(1)
					}
					envvars["OPENAI_API_KEY"] = apikey
				}
				break
			}
		}
	}

	// get jina and serper api keys
	envvars["JINA_API_KEY"] = getUserInput("Enter your Jina API key (optional, just press enter for skipping it)", true)
	envvars["SERPER_API_KEY"] = getUserInput("Enter your Serper API key (optional, just press enter for skipping it)", true)
	fmt.Printf("\n")

	// check ollama requirement
	OLLAMA_REQUIRED := false
	for _, model := range strings.Split(envvars["DKN_MODELS"], ",") {
		for _, ollama_model := range OLLAMA_MODELS {
			if model == ollama_model {
				OLLAMA_REQUIRED = true
				break
			}
		}
	}

	// check ollama environment
	if OLLAMA_REQUIRED {
		ollamaHost, ollamaPort, dockerNetworkMode, composeProfile := handleOllamaEnv(envvars["OLLAMA_HOST"], envvars["OLLAMA_PORT"], *dockerOllama)
		envvars["OLLAMA_HOST"] = ollamaHost
		envvars["OLLAMA_PORT"] = ollamaPort
		envvars["COMPOSE_PROFILES"] = composeProfile
		envvars["DKN_DOCKER_NETWORK_MODE"] = dockerNetworkMode
	
		fmt.Printf("Ollama host: %s (network mode: %s)\n", envvars["OLLAMA_HOST"], envvars["DKN_DOCKER_NETWORK_MODE"])
	} else {
		fmt.Println("No Ollama model provided. Skipping the Ollama execution")
	}

	// log level
	if *dev {
		envvars["RUST_LOG"] = "none,dkn_compute=debug,ollama_workflows=info"
	} else if *trace {
		envvars["RUST_LOG"] = "none,dkn_compute=trace"
	} else {
		// default level info
		envvars["RUST_LOG"] = "none,dkn_compute=info"
	}

	// Update the image
	fmt.Println("\nPulling the latest compute node image...")
	_, err = runCommand(true, true, []string{"DOCKER_CLI_HINTS=false"}, "docker", "pull", "firstbatch/dkn-compute-node:latest")
	if err != nil {
		fmt.Println("Error during pulling the latest compute node image")
		exitWithDelay(1)
	}

	// dump the final env
	godotenv.Write(envvars, filepath.Join(WORKING_DIR, ".env"))

	if *background {
		fmt.Printf("\nStarting in BACKGROUND mode...\n")
	} else {
		fmt.Printf("\nStarting in FOREGROUND mode...\n")
	}
	fmt.Printf("Log level: %s\n", envvars["RUST_LOG"])
	fmt.Printf("Models: %s\n", envvars["DKN_MODELS"])
	fmt.Printf("Operating System: %s\n", runtime.GOOS)
	fmt.Printf("COMPOSE_PROFILES: %s\n\n", envvars["COMPOSE_PROFILES"])

	// Run docker-compose up
	_, err = runCommand(true, true, mapToList(envvars), composeCommand, composeUpArgs...)
	if err != nil {
		fmt.Printf("ERROR: docker-compose, %s", err)
		exitWithDelay(1)
	}

	fmt.Println("All good! Compute node is up and running.")
	fmt.Println("You can check logs with: docker compose logs -f compute.")

	// Foreground mode
	if !(*background) {
		fmt.Println("\nUse Control-C to exit")
		sig := make(chan os.Signal, 1)
		signal.Notify(sig, os.Interrupt)
		<-sig

		fmt.Println("\nShutting down...")
		_, err = runCommand(true, true, mapToList(envvars), composeCommand, composeDownArgs...)
		if err != nil {
			fmt.Printf("Error during docker compose down; %s\n", err)
		}

		fmt.Println("\nbye")
		os.Exit(0)
	}
}
