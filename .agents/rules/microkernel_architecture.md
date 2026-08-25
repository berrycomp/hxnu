---
name: microkernel-architecture
description: Enforces microkernel design principles for HXNU, strictly prohibiting drivers inside the core kernel repo.
---
# Microkernel Architecture Rules

1. **Microkernel Design**: HXNU is a pure microkernel. Do not treat it as a monolithic kernel.
2. **Driver Isolation**: Hardware drivers MUST NOT be developed, placed, or compiled inside the core kernel repository (\hxnu\). 
3. **External Repositories**: Every hardware driver must be created and maintained in its own separate, independent repository.
