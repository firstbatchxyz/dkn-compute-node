# 🚀 LLM Node Runner's Guide: Minimum Specs

Hello, Drians! 👋 Here's a guide to help you understand the minimum specs needed for running different LLMs. We've broken it down into two main categories: (1) **GPU-enabled** nodes and (2) **CPU-only** nodes, as you can run your nodes on machines both _with_ or _without_ GPU.

- ## 🖥️ GPU-Enabled Nodes

These specs are based on a system with 16 CPUs and 64GB RAM.

| Model          | GPU Memory     | CPU Usage (cores) | RAM Usage    |
| -------------- | -------------- | ----------------- | ------------ |
| Llama3_1_8B    | 6.1 - 6.2 GB   | 8.6 - 12.8 cores  | 8.5 GB       |
| Phi3Mini       | 3.3 - 3.4 GB   | 14.4 - 22.5 cores | 7.7 GB       |
| Phi3Medium128k | 10.9 - 11.0 GB | 7.9 - 11.4 cores  | 5.3 GB       |
| Phi3Medium     | 10.9 - 11.0 GB | 4.3 - 5.7 cores   | 5.3 GB       |
| NousTheta      | 9.6 GB         | 4.1 - 4.8 cores   | 6.4 - 6.6 GB |

- ## 💻 CPU-Only Nodes

For those running without a GPU, we've got you covered too! Here are the specs for different CPU types:

### ARM (4 CPU, 16GB RAM)

| Model          | CPU Usage (cores) | RAM Usage     |
| -------------- | ----------------- | ------------- |
| NousTheta      | 3.0 - 3.5 cores   | 9.6 GB        |
| Phi3Medium     | 3.7 - 3.8 cores   | 10.4 GB       |
| Phi3Medium128k | 3.7 - 3.8 cores   | 10.4 GB       |
| Phi3Mini       | 3.2 - 6.1 cores   | 5.6 - 11.4 GB |
| Llama3_1_8B    | 3.4 - 3.7 cores   | 6.1 GB        |

### ARM (8 CPU, 16GB RAM)

| Model          | CPU Usage (cores) | RAM Usage     |
| -------------- | ----------------- | ------------- |
| NousTheta      | 6.2 - 6.3 cores   | 9.6 GB        |
| Phi3Medium     | 6.5 cores         | 10.8 GB       |
| Phi3Medium128k | 6.5 cores         | 10.8 GB       |
| Phi3Mini       | 5.4 - 7.0 cores   | 5.8 - 11.6 GB |
| Llama3_1_8B    | 3.4 - 4.2 cores   | 6.2 GB        |

### AMD (8 CPU, 16GB RAM)

| Model          | CPU Usage (cores) | RAM Usage     |
| -------------- | ----------------- | ------------- |
| NousTheta      | 2.3 - 3.2 cores   | 9.5 GB        |
| Phi3Medium     | 3.3 - 3.4 cores   | 10.3 GB       |
| Phi3Medium128k | 1.6 - 3.2 cores   | 10.2 GB       |
| Phi3Mini       | 2.8 - 3.1 cores   | 5.4 - 11.4 GB |
| Llama3_1_8B    | 4.5 - 4.6 cores   | 11.1 GB       |

### Intel (8 CPU, 16GB RAM)

| Model          | CPU Usage (cores) | RAM Usage     |
| -------------- | ----------------- | ------------- |
| NousTheta      | 2.3 - 2.9 cores   | 9.7 GB        |
| Phi3Medium     | 3.1 - 3.3 cores   | 10.4 GB       |
| Phi3Medium128k | 2.2 - 3.3 cores   | 10.3 GB       |
| Phi3Mini       | 2.6 - 4.1 cores   | 5.4 - 11.0 GB |
| Llama3_1_8B    | 3.7 - 3.9 cores   | 11.3 GB       |

## 📝 Notes

- CPU usage can vary significantly between tasks, especially for long context vs. multiple steps.

- Some models may require more than the available CPU cores, which could lead to slower performance.

- RAM usage is generally consistent but can spike for certain operations.

- **Important**: For systems with 4 CPUs and 8GB RAM, only Phi3Mini was able to run successfully.\*\*

- **Important**: Lower CPU count results in lower performance. Systems with fewer CPUs will process requests more slowly, especially for models that require more CPU resources than are available.

Remember, these are minimum specs, and your experience may vary depending on the specific tasks and workload. Happy node running! 🎉
