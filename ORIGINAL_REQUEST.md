# Original User Request

## Initial Request — 2026-08-04T01:54:43Z

Integrate the MIT edition of rustybox into the rootfs as the primary userspace utility suite, ensure the display resolution is maximized dynamically during the transition to userspace, and add Multilingual (Turkish/Russian) font support.

Working directory: /mnt/KINGSTON/Berturk/Projects/hxnu
Integrity mode: development

## Requirements

### R1. Dynamic Framebuffer Resolution Maximization
Implement a dynamic framebuffer/display driver component within the kernel or `heterexec` that automatically probes, negotiates, and sets the highest available display resolution during the transition from early kernel boot to userspace.

### R2. rustybox (MIT Edition) Integration
Integrate the MIT-licensed edition of `rustybox` into the `rootfs`. The source code MUST be fetched and built natively as part of the existing custom CMake and Rustc sequential build system, ensuring it is statically compiled for the target bare-metal architecture.

### R3. Multilingual CLI Font Support
The userspace command-line interface (CLI) and framebuffer terminal must support rendering Turkish and Russian (Cyrillic) characters via UTF-8. A suitable glyph renderer or PC Screen Font (PSF) must be loaded to handle these characters correctly.

### R4. Resource Constraints (Lite Team)
Due to strict API rate limits, the teamwork multi-agent system MUST operate with a reduced footprint. Skip the heavy Challenger and Reviewer phases. Rely on a strict, minimal Orchestrator -> Worker -> Auditor pipeline.

## Acceptance Criteria

### Execution & Integration Verification
- [ ] Running the QEMU test environment successfully builds `rustybox` from source via the CMake pipeline without manual intervention.
- [ ] The kernel/heterexec logs demonstrate that the display resolution is dynamically updated to the maximum supported hardware limit immediately prior to or during the userspace handoff.
- [ ] The `rootfs` correctly contains the `rustybox` binary, and userspace successfully executes a `rustybox` command (like `sh` or `ls`) without crashing.
- [ ] The CLI/Framebuffer successfully renders a test string containing Turkish ("Ş, Ğ, Ç, Ö, Ü, İ") and Russian ("А, Б, В, Г, Д") characters.
