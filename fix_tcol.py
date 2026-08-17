import os

header_rs = "// TCOL / HPL (HXNU Public License)\n"
header_cmake = "# TCOL / HPL (HXNU Public License)\n"

files = [
    "./boot/gboot/src/drivers/alveo_sim.rs",
    "./boot/gboot/src/drivers/gui.rs",
    "./boot/gboot/src/drivers/mod.rs",
    "./boot/gboot/src/drivers/touch.rs",
    "./boot/gboot/src/main.rs",
    "./boot/gboot/src/rk3588_regs.rs",
    "./boot/gboot/src/pcie_ep.rs",
    "./boot/gboot/src/handover.rs",
    "./boot/gboot/CMakeLists.txt",
    "./CMakeLists.txt"
]

for f in files:
    if os.path.exists(f):
        with open(f, 'r', encoding='utf-8') as file:
            content = file.read()
        
        if "TCOL / HPL (HXNU Public License)" in content:
            continue
            
        header = header_cmake if f.endswith("CMakeLists.txt") else header_rs
        
        with open(f, 'w', encoding='utf-8') as file:
            file.write(header + content)
        print(f"Fixed {f}")
    else:
        print(f"Not found: {f}")
