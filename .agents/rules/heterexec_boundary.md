---
description: Architectural boundary between Pardus and Neonix regarding heterexec.
trigger: always_on
---

# Rule: Heterexec and Neonix Boundary

- **Context:** `heterexec` and `Neonix` are a separate HXNU-based system. They are **NOT** part of the standard Pardus 27 kernel branch.
- **Constraint:** When working on general HXNU or Pardus tasks (such as fixing kernel bugs on branches like `fix-iter4`), **DO NOT** attempt to fix, run, or analyze `heterexec` components (e.g., `test-heterexec.sh` or `SUPERNOVA_MMIO_BASE` doorbell issues) unless the user explicitly directs you to work on Neonix/heterexec.
- **Action:** Ignore `heterexec`-related failures in handoff reports or logs if the current context is Pardus.
