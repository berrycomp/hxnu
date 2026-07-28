import re
with open("/home/eilhanzy/Projects/hxnu/.agents/orchestrator/PROJECT.md", "r") as f:
    text = f.read()

text = text.replace("| 4 | Supernova Driver | Bridge HPS, HXNU scheduler, and Nvidia GSP (GPU System Processor). Handle extreme-speed synchronization to ensure zero-latency dispatching without timeouts. | M1, M2, M3 | PLANNED |", "| 4 | Supernova Driver | Bridge HPS, HXNU scheduler, and Nvidia GSP (GPU System Processor). Handle extreme-speed synchronization to ensure zero-latency dispatching without timeouts. | M1, M2, M3 | DONE |")
text = text.replace("| 5 | Syscall Entropy-Inclusive IDs | Add entropy-inclusive ID generation and validation to the Syscall layer for secure, collision-free dispatching across UMA. | M2, M3 | PLANNED |", "| 5 | Syscall Entropy-Inclusive IDs | Add entropy-inclusive ID generation and validation to the Syscall layer for secure, collision-free dispatching across UMA. | M2, M3 | DONE |")
text = text.replace("| 7 | MaRTix Core | Implement MaRTix natively inside the Supernova Driver to route matrix operations directly to RT Cores. | M1 | PLANNED |", "| 7 | MaRTix Core | Implement MaRTix natively inside the Supernova Driver to route matrix operations directly to RT Cores. | M1 | DONE |")

with open("/home/eilhanzy/Projects/hxnu/.agents/orchestrator/PROJECT.md", "w") as f:
    f.write(text)
