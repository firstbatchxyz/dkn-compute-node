# 🚀 LLM Node Runner's Guide: Minimum Specs

Hello, Drians! 👋 Here's a guide to help you understand the minimum specs needed for running different LLMs. We've broken it down into two main categories: (1) **GPU-enabled** nodes and (2) **CPU-only** nodes, as you can run your nodes on machines both _with_ or _without_ GPU.

- ## 🖥️ GPU-Enabled Nodes

### RTX3090 Single GPU:

| Model                               | TPS      |
| ----------------------------------- | -------- |
| finalend/hermes-3-llama-3.1:8b-q8_0 | 76.4388  |
| phi3:14b-medium-4k-instruct-q4_1    | 75.6148  |
| phi3:14b-medium-128k-instruct-q4_1  | 76.0658  |
| phi3.5:3.8b                         | 195.0728 |
| phi3.5:3.8b-mini-instruct-fp16      | 88.4656  |
| gemma2:9b-instruct-q8_0             | 56.2726  |
| gemma2:9b-instruct-fp16             | 37.9404  |
| llama3.1:latest                     | 103.3473 |
| llama3.1:8b-instruct-q8_0           | 78.5861  |
| llama3.1:8b-instruct-fp16           | 50.9302  |
| llama3.1:8b-text-q4_K_M             | 104.4776 |
| llama3.1:8b-text-q8_0               | 82.3980  |
| llama3.2:1b                         | 293.1785 |
| llama3.2:3b                         | 168.7500 |
| llama3.2:1b-text-q4_K_M             | 349.2497 |
| qwen2.5:7b-instruct-q5_0            | 114.0511 |
| qwen2.5:7b-instruct-fp16            | 53.5423  |
| qwen2.5-coder:1.5b                  | 238.6117 |
| qwen2.5-coder:7b-instruct           | 125.2194 |
| qwen2.5-coder:7b-instruct-q8_0      | 83.7696  |
| qwen2.5-coder:7b-instruct-fp16      | 53.7400  |
| qwq                                 | 33.4434  |
| deepseek-coder:6.7b                 | 141.7769 |
| deepseek-r1:1.5b                    | 235.8560 |
| deepseek-r1:7b                      | 121.9637 |
| deepseek-r1:8b                      | 107.5933 |
| deepseek-r1:14b                     | 66.5972  |
| deepseek-r1:32b                     | 34.4669  |
| deepseek-r1                         | 120.9809 |
| driaforall/tiny-agent-a:0.5b        | 279.2553 |
| driaforall/tiny-agent-a:1.5b        | 201.7011 |
| driaforall/tiny-agent-a:3b          | 135.1052 |

### H200 SXM Single GPU:

| Model                               | TPS      |
| ----------------------------------- | -------- |
| finalend/hermes-3-llama-3.1:8b-q8_0 | 121.2871 |
| phi3:14b-medium-4k-instruct-q4_1    | 128.9496 |
| phi3:14b-medium-128k-instruct-q4_1  | 124.4223 |
| phi3.5:3.8b                         | 184.3729 |
| phi3.5:3.8b-mini-instruct-fp16      | 155.6164 |
| gemma2:9b-instruct-q8_0             | 91.6370  |
| gemma2:9b-instruct-fp16             | 85.6672  |
| llama3.1:latest                     | 123.8938 |
| llama3.1:8b-instruct-q8_0           | 112.3102 |
| llama3.1:8b-instruct-fp16           | 108.9053 |
| llama3.1:8b-text-q4_K_M             | 148.0687 |
| llama3.1:8b-text-q8_0               | 135.3251 |
| llama3.1:70b-instruct-q4_0          | 47.0107  |
| llama3.1:70b-instruct-q8_0          | 35.2827  |
| llama3.2:1b                         | 163.9058 |
| llama3.2:3b                         | 150.6063 |
| llama3.3:70b                        | 39.1993  |
| llama3.2:1b-text-q4_K_M             | 233.6957 |
| qwen2.5:7b-instruct-q5_0            | 126.5432 |
| qwen2.5:7b-instruct-fp16            | 103.8552 |
| qwen2.5:32b-instruct-fp16           | 40.3735  |
| qwen2.5-coder:1.5b                  | 187.3554 |
| qwen2.5-coder:7b-instruct           | 119.7279 |
| qwen2.5-coder:7b-instruct-q8_0      | 108.9536 |
| qwen2.5-coder:7b-instruct-fp16      | 104.0222 |
| qwq                                 | 59.4734  |
| deepseek-coder:6.7b                 | 136.8015 |
| mixtral:8x7b                        | 94.9618  |
| deepseek-r1:1.5b                    | 160.8217 |
| deepseek-r1:7b                      | 141.2172 |
| deepseek-r1:8b                      | 136.8324 |
| deepseek-r1:14b                     | 90.3022  |
| deepseek-r1:32b                     | 63.1900  |
| deepseek-r1:70b                     | 39.4153  |
| deepseek-r1                         | 121.8406 |
| driaforall/tiny-agent-a:0.5b        | 148.5390 |
| driaforall/tiny-agent-a:1.5b        | 180.9409 |
| driaforall/tiny-agent-a:3b          | 111.1869 |

- ## 💻 CPU-Only Nodes

For those running without a GPU, we've got you covered too! Here are the specs for different CPU types:

### AMD (8 CPU, 16GB RAM)

| Model                        | TPS     |
| ---------------------------- | ------- |
| llama3.2:1b                  | 22.6293 |
| llama3.2:1b-text-q4_K_M      | 25.0413 |
| qwen2.5-coder:1.5b           | 21.7418 |
| deepseek-r1:1.5b             | 29.7842 |
| driaforall/tiny-agent-a:0.5b | 54.5455 |
| driaforall/tiny-agent-a:1.5b | 19.9501 |

### AMD (16 CPU, 32GB RAM)

| Model                        | TPS     |
| ---------------------------- | ------- |
| phi3.5:3.8b                  | 15.3677 |
| llama3.2:1b                  | 25.6367 |
| llama3.2:3b                  | 16.3185 |
| llama3.2:1b-text-q4_K_M      | 38.0039 |
| qwen2.5-coder:1.5b           | 30.3651 |
| deepseek-r1:1.5b             | 30.2977 |
| driaforall/tiny-agent-a:0.5b | 61.2553 |
| driaforall/tiny-agent-a:1.5b | 25.7011 |

### AMD (32 CPU, 64GB RAM)

| Model                        | TPS     |
| ---------------------------- | ------- |
| phi3.5:3.8b                  | 22.9944 |
| llama3.2:1b                  | 40.6091 |
| llama3.2:3b                  | 26.0240 |
| llama3.2:1b-text-q4_K_M      | 56.2027 |
| qwen2.5-coder:1.5b           | 44.6331 |
| deepseek-coder:6.7b          | 15.1620 |
| deepseek-r1:1.5b             | 43.8323 |
| driaforall/tiny-agent-a:0.5b | 59.9854 |
| driaforall/tiny-agent-a:1.5b | 27.7891 |

### AMD (48 CPU, 96GB RAM)

| Model                        | TPS     |
| ---------------------------- | ------- |
| phi3.5:3.8b                  | 29.7455 |
| llama3.1:latest              | 17.4744 |
| llama3.1:8b-text-q4_K_M      | 18.1928 |
| llama3.2:1b                  | 49.1555 |
| llama3.2:3b                  | 33.9283 |
| llama3.2:1b-text-q4_K_M      | 72.7273 |
| qwen2.5:7b-instruct-q5_0     | 17.0779 |
| qwen2.5-coder:1.5b           | 56.2710 |
| qwen2.5-coder:7b-instruct    | 18.2935 |
| deepseek-coder:6.7b          | 21.2014 |
| deepseek-r1:1.5b             | 55.0080 |
| deepseek-r1:7b               | 18.0150 |
| deepseek-r1:8b               | 16.4574 |
| deepseek-r1                  | 18.0991 |
| driaforall/tiny-agent-a:0.5b | 86.2903 |
| driaforall/tiny-agent-a:1.5b | 41.6198 |
| driaforall/tiny-agent-a:3b   | 24.1364 |

### AMD (64 CPU, 128GB RAM)

| Model                        | TPS     |
| ---------------------------- | ------- |
| phi3.5:3.8b                  | 33.8993 |
| llama3.1:latest              | 19.3015 |
| llama3.1:8b-text-q4_K_M      | 19.9081 |
| llama3.2:1b                  | 55.6815 |
| llama3.2:3b                  | 36.6654 |
| llama3.2:1b-text-q4_K_M      | 68.9655 |
| qwen2.5:7b-instruct-q5_0     | 18.0591 |
| qwen2.5-coder:1.5b           | 56.7301 |
| qwen2.5-coder:7b-instruct    | 20.1563 |
| deepseek-coder:6.7b          | 23.4261 |
| deepseek-r1:1.5b             | 57.0494 |
| deepseek-r1:7b               | 20.3577 |
| deepseek-r1:8b               | 18.6653 |
| deepseek-r1                  | 20.2571 |
| driaforall/tiny-agent-a:0.5b | 94.6503 |
| driaforall/tiny-agent-a:1.5b | 49.5431 |
| driaforall/tiny-agent-a:3b   | 27.1564 |

### AMD (96 CPU, 192GB RAM)

| Model                        | TPS     |
| ---------------------------- | ------- |
| phi3.5:3.8b                  | 34.1058 |
| llama3.1:latest              | 20.2221 |
| llama3.1:8b-text-q4_K_M      | 20.1473 |
| llama3.2:1b                  | 54.5232 |
| llama3.2:3b                  | 37.6344 |
| llama3.2:1b-text-q4_K_M      | 65.7570 |
| qwen2.5:7b-instruct-q5_0     | 20.2058 |
| qwen2.5-coder:1.5b           | 55.4435 |
| qwen2.5-coder:7b-instruct    | 21.3058 |
| deepseek-coder:6.7b          | 24.6414 |
| deepseek-r1:1.5b             | 54.3133 |
| deepseek-r1:7b               | 20.8902 |
| deepseek-r1:8b               | 18.7142 |
| deepseek-r1                  | 22.1564 |
| driaforall/tiny-agent-a:0.5b | 94.7864 |
| driaforall/tiny-agent-a:1.5b | 50.7868 |
| driaforall/tiny-agent-a:3b   | 29.4635 |

### AMD (192 CPU, 384GB RAM)

| Model                               | TPS     |
| ----------------------------------- | ------- |
| finalend/hermes-3-llama-3.1:8b-q8_0 | 16.8002 |
| phi3.5:3.8b                         | 26.2855 |
| phi3.5:3.8b-mini-instruct-fp16      | 16.7343 |
| llama3.1:latest                     | 21.9456 |
| llama3.1:8b-instruct-q8_0           | 16.7135 |
| llama3.1:8b-text-q4_K_M             | 22.5764 |
| llama3.1:8b-text-q8_0               | 16.3817 |
| llama3.2:1b                         | 43.5632 |
| llama3.2:3b                         | 29.5560 |
| llama3.2:1b-text-q4_K_M             | 48.6348 |
| qwen2.5:7b-instruct-q5_0            | 21.4938 |
| qwen2.5-coder:1.5b                  | 33.3333 |
| qwen2.5-coder:7b-instruct           | 21.7933 |
| qwen2.5-coder:7b-instruct-q8_0      | 17.8134 |
| deepseek-coder:6.7b                 | 23.4474 |
| deepseek-r1:1.5b                    | 32.7795 |
| deepseek-r1:7b                      | 22.5376 |
| deepseek-r1:8b                      | 20.3057 |
| deepseek-r1                         | 23.0604 |
| driaforall/tiny-agent-a:0.5b        | 42.1866 |
| driaforall/tiny-agent-a:1.5b        | 33.4957 |
| driaforall/tiny-agent-a:3b          | 24.5138 |

### ARM (192 CPU, 384GB RAM)

| Model                        | TPS     |
| ---------------------------- | ------- |
| phi3.5:3.8b                  | 26.3062 |
| llama3.1:latest              | 18.9597 |
| llama3.1:8b-text-q4_K_M      | 18.2489 |
| llama3.2:1b                  | 43.7856 |
| llama3.2:3b                  | 30.3443 |
| llama3.2:1b-text-q4_K_M      | 49.6852 |
| qwen2.5:7b-instruct-q5_0     | 16.8128 |
| qwen2.5-coder:1.5b           | 38.3562 |
| qwen2.5-coder:7b-instruct    | 19.5582 |
| deepseek-coder:6.7b          | 21.2699 |
| deepseek-r1:1.5b             | 36.0020 |
| deepseek-r1:7b               | 19.5293 |
| deepseek-r1:8b               | 18.5300 |
| deepseek-r1                  | 18.9405 |
| driaforall/tiny-agent-a:0.5b | 28.4991 |
| driaforall/tiny-agent-a:1.5b | 31.6353 |
| driaforall/tiny-agent-a:3b   | 22.2788 |

## 📝 Notes

- CPU usage can vary significantly between tasks, especially for long context vs. multiple steps.

- Some models may require more than the available CPU cores, which could lead to slower performance.

- RAM usage is generally consistent but can spike for certain operations.

- **Important**: Lower CPU count results in lower performance. Systems with fewer CPUs will process requests more slowly, especially for models that require more CPU resources than are available.

Remember, these are minimum specs, and your experience may vary depending on the specific tasks and workload. Happy node running! 🎉
