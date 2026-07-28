with open("src/syscall.rs", "r") as f:
    lines = f.readlines()

new_lines = []
skip = False
for i, line in enumerate(lines):
    if "HPS_SYS_SPAWN_WORKER => {" in line:
        new_lines.append(line)
        new_lines.append("            // Bridge to sched.rs hooks\n")
        new_lines.append("            crate::sched::trigger_hps_bridge(args[0], args[1] != 0);\n")
        new_lines.append("            SyscallOutcome::success(0)\n")
        skip = True
    elif skip and "}," in line:
        new_lines.append("        },\n")
        skip = False
    elif not skip:
        new_lines.append(line)

with open("src/syscall.rs", "w") as f:
    f.writelines(new_lines)
