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

func checkDockerCompose() string {
	commands := []string{"docker compose", "docker-compose"}
	for _, cmd := range commands {
		if _, err := exec.Command(cmd, "version").Output(); err == nil {
			return cmd
		}
	}
	fmt.Println("docker compose is not installed on this machine. It's required to run the node.")
	fmt.Println("Check https://docs.docker.com/compose/install/ for installation.")
	os.Exit(1)
	return ""
}

func checkRequiredEnvVars(envvars map[string]string) {
	absent_env := false
	if envvars["DKN_WALLET_SECRET_KEY"] == "" {
		fmt.Println("DKN_WALLET_SECRET_KEY env-var is not set, getting it interactively")
		skey, err := getDknSecretKey()
		if err != nil {
			fmt.Printf("Error during user input: %s\n", err)
			os.Exit(1)
		}
		envvars["DKN_WALLET_SECRET_KEY"] = skey
		absent_env = true
	}

	if envvars["DKN_ADMIN_PUBLIC_KEY"] == "" {
		fmt.Println("DKN_ADMIN_PUBLIC_KEY env-var is not set, getting it interactively")
		akey, err := getDknAdminPublicKey()
		if err != nil {
			fmt.Printf("Error during user input: %s\n", err)
			os.Exit(1)
		}
		envvars["DKN_ADMIN_PUBLIC_KEY"] = akey
		absent_env = true
	}

	if absent_env {
		// dump the .env file for future usages
		fmt.Printf("Dumping the given env-vars to .env\n\n")
		godotenv.Write(envvars, "./.env")

	}
}

func getDknSecretKey() (string, error) {
	reader := bufio.NewReader(os.Stdin)
	// get DKN_WALLET_SECRET_KEY
	fmt.Print("Please enter your DKN Wallet Secret Key (32-bytes hex encoded): ")
	skey, err := reader.ReadString('\n')
	if err != nil {
		return "", fmt.Errorf("couldn't get DKN Wallet Secret Key")
	}
	skey = strings.Split(skey, "\n")[0]
	skey = strings.TrimSpace(skey)
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

func getDknAdminPublicKey() (string, error) {
	reader := bufio.NewReader(os.Stdin)

	// get DKN_ADMIN_PUBLIC_KEY
	fmt.Print("Please enter DKN Admin Public Key (32-bytes hex encoded, you can get it from .env.example): ")
	admin_pkey, err := reader.ReadString('\n')
	if err != nil {
		return "", fmt.Errorf("couldn't get DKN Admin Public Key")
	}
	admin_pkey = strings.Split(admin_pkey, "\n")[0]
	admin_pkey = strings.TrimSpace(admin_pkey)
	admin_pkey = strings.TrimPrefix(admin_pkey, "0x")
	// decode the hex string into bytes
	decoded_admin_pkey, err := hex.DecodeString(admin_pkey)
	if err != nil {
		return "", fmt.Errorf("DKN Admin Public Key should be 32-bytes hex encoded")
	}
	// ensure the decoded bytes are exactly 33 bytes
	if len(decoded_admin_pkey) != 33 {
		return "", fmt.Errorf("DKN Admin Public Key should be 33 bytes long")
	}
	return admin_pkey, nil
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
	OLLAMA_MODELS       = []string{"nous-hermes2theta-llama3-8b", "phi3:medium", "phi3:medium-128k", "phi3:3.8b", "llama3.1:latest"}
	DEFAULT_OLLAMA_PORT = 11434
	OLLAMA_REQUIRED     = false
	DOCKER_HOST         = "http://host.docker.internal"
	LOCAL_HOST          = "http://localhost"
	OLLAMA_MAX_RETRIES  = 5
	COMPOSE_PROFILES    = []string{}

	// this is the default network mode, but
	// based on local Ollama & OS we may set it to `host`
	// https://docs.docker.com/engine/network/#drivers
	DKN_DOCKER_NETWORK_MODE = "bridge"
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
	flag.Parse()

	// Display help and exit if -h or --help is provided
	if *help {
		flag.Usage()
		os.Exit(0)
	}

	fmt.Printf("Setting up the environment...\n\n")

	// Check Docker Compose
	composeCommand := checkDockerCompose()

	// Load .env file if exists
	envvars, err := godotenv.Read("./.env")
	if err != nil {
		// if .env is not exists or required vars are absent, get them from terminal
		fmt.Println("Couldn't load .env file")
		fmt.Println("Getting required env vars interactively")
		skey, err := getDknSecretKey()
		if err != nil {
			fmt.Printf("Error during user input: %s\n", err)
			os.Exit(1)
		}

		akey, err := getDknAdminPublicKey()
		if err != nil {
			fmt.Printf("Error during user input: %s\n", err)
			os.Exit(1)
		}

		envvars["DKN_WALLET_SECRET_KEY"] = skey
		envvars["DKN_ADMIN_PUBLIC_KEY"] = akey
		// dump the .env file for future usages
		fmt.Printf("Dumping the given env-vars to .env\n\n")
		godotenv.Write(envvars, "./.env")

	}

	checkRequiredEnvVars(envvars)

	// use models given with -m flag
	if len(models) != 0 {
		envvars["DKN_MODELS"] = strings.Join(models, ",")
	}

	// check ollama models
	for _, model := range strings.Split(envvars["DKN_MODELS"], ",") {
		for _, ollama_model := range OLLAMA_MODELS {
			if model == ollama_model {
				OLLAMA_REQUIRED = true
				break
			}
		}
	}

	// check ollama
	if OLLAMA_REQUIRED {
		// local ollama
		if !(*dockerOllama) {
			if isCommandAvailable("ollama") {
				// host machine has ollama installed
				// we first going to check whether its serving or not
				// if not script runs ollama serve command manually and stores its pid

				// prepare local ollama url
				if envvars["OLLAMA_HOST"] == "" || envvars["OLLAMA_HOST"] == DOCKER_HOST {
					// we have to check Ollama at host, but if the given host is
					// host.docker.internal we still have to check the localhost
					// here, we construct `ollama_url` with respect to that
					envvars["OLLAMA_HOST"] = LOCAL_HOST
				}
				if envvars["OLLAMA_PORT"] == "" {
					envvars["OLLAMA_PORT"] = strconv.Itoa(DEFAULT_OLLAMA_PORT)
				}

				// check is it already serving
				if isOllamaServing(envvars["OLLAMA_HOST"], envvars["OLLAMA_PORT"]) {
					fmt.Printf("Local Ollama is already up at %s:%s and running, using it\n", envvars["OLLAMA_HOST"], envvars["OLLAMA_PORT"])
				} else {
					// ollama is not live, so we launch it ourselves
					fmt.Println("Local Ollama is not live, running ollama serve")
					ollama_pid, err := runOllamaServe(envvars["OLLAMA_HOST"], envvars["OLLAMA_PORT"])
					if err != nil {
						// ollama failed to start, exit
						fmt.Println(err)
						fmt.Println("You can use the --docker-ollama flag to use the Docker Ollama image instead.")
						os.Exit(1)
					} else {
						fmt.Printf("Local Ollama server is up at %s:%s and running with PID %d\n", envvars["OLLAMA_HOST"], envvars["OLLAMA_PORT"], ollama_pid)
					}
				}

				// to use the local Ollama, we need to configure the network depending on the Host
				// Windows and Mac should work with host.docker.internal alright,
				// but Linux requires `host` network mode with `localhost` as the Host URL
				if runtime.GOOS == "darwin" {
					envvars["OLLAMA_HOST"] = DOCKER_HOST
				} else if runtime.GOOS == "linux" {
					envvars["OLLAMA_HOST"] = LOCAL_HOST
					DKN_DOCKER_NETWORK_MODE = "host"
					// } else if runtime.GOOS == "windows" {
					// 	// TODO test for windows
					// 	envvars["OLLAMA_HOST"] = LOCAL_HOST
				}
			} else {
				// although --docker-ollama was not passed, we checked and couldnt find Ollama
				// so we will use Docker anyways
				fmt.Println("Ollama is not installed on this machine, will use Docker Ollama service.")
				*dockerOllama = true
			}
		}

		if *dockerOllama {
			// using docker-ollama, check profiles
			if isCommandAvailable("nvidia-smi") {
				COMPOSE_PROFILES = append(COMPOSE_PROFILES, "ollama-cuda")
				fmt.Println("GPU type detected: CUDA")
			} else if isCommandAvailable("rocminfo") {
				fmt.Println("GPU type detected: ROCM")
				COMPOSE_PROFILES = append(COMPOSE_PROFILES, "ollama-rocm")
			} else {
				fmt.Println("No GPU found, using ollama-cpu")
				COMPOSE_PROFILES = append(COMPOSE_PROFILES, "ollama-cpu")
			}

			// use docker internal for the Ollama host
			envvars["OLLAMA_HOST"] = DOCKER_HOST
			envvars["OLLAMA_PORT"] = strconv.Itoa(DEFAULT_OLLAMA_PORT)
			DKN_DOCKER_NETWORK_MODE = "bridge"
		}

		fmt.Printf("Ollama host: %s (network mode: %s)\n", envvars["OLLAMA_HOST"], DKN_DOCKER_NETWORK_MODE)
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
		os.Exit(1)
	}

	// set runtime env vars
	envvars["COMPOSE_PROFILES"] = strings.Join(COMPOSE_PROFILES, ",")
	envvars["DKN_DOCKER_NETWORK_MODE"] = DKN_DOCKER_NETWORK_MODE

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
	_, err = runCommand(true, true, mapToList(envvars), composeCommand, "up", "-d")
	if err != nil {
		fmt.Printf("ERROR: docker-compose, %s", err)
		os.Exit(1)
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
		_, err = runCommand(true, true, mapToList(envvars), composeCommand, "down")
		if err != nil {
			fmt.Printf("Error during docker compose down; %s\n", err)
		}

		fmt.Println("\nbye")
		os.Exit(0)
	}
}
