---
description: Build system and toolchain constraints. Enforces CMake usage and custom toolchain readiness.
trigger: always_on
---

# Rule: Build System and Toolchain

- **Build System:** We exclusively adopt CMake for all compilation and build tasks in this repository. Do not use raw Makefiles or direct compiler invocations (e.g., calling `gcc` or `rustc` directly from bash scripts) to build core components.
- **CMake Concurrency:** All shell scripts orchestrating builds should invoke CMake (e.g., `cmake --build .`). 
- **Custom Toolchain Readiness:** In the future, a custom toolchain will likely be required. All build configurations must avoid hardcoding compiler paths (e.g., never hardcode `/usr/bin/gcc`).
- **Toolchain File:** Always respect `CMAKE_TOOLCHAIN_FILE`, `CMAKE_C_COMPILER`, `CMAKE_CXX_COMPILER`, and standard environment variables (`CC`, `CXX`) to ensure seamless transition to a custom toolchain when it arrives.
