import re

with open('CMakeLists.txt', 'r') as f:
    cmake = f.read()

cmake = cmake.replace('''        --crate-type staticlib
        -C opt-level=3
        -C panic=abort
        -C relocation-model=static
        ${CMAKE_CURRENT_SOURCE_DIR}/kernel/src/main.rs''', '''        --emit obj
        -C opt-level=3
        -C panic=abort
        -C relocation-model=static
        -C link-arg=-nostdlib
        ${CMAKE_CURRENT_SOURCE_DIR}/kernel/src/main.rs''')

cmake = cmake.replace('''        --crate-type staticlib
        -C opt-level=3
        -C panic=abort
        -C relocation-model=static
        ${GBOOT_CFG}''', '''        --emit obj
        -C opt-level=3
        -C panic=abort
        -C relocation-model=static
        -C link-arg=-nostdlib
        ${GBOOT_CFG}''')

cmake = cmake.replace('OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/libhxnu_kernel.a ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o', 'OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o')
cmake = cmake.replace('-o ${CMAKE_CURRENT_BINARY_DIR}/libhxnu_kernel.a', '-o ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o')
cmake = cmake.replace('COMMAND cp ${CMAKE_CURRENT_BINARY_DIR}/libhxnu_kernel.a ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o\n', '')
cmake = cmake.replace('DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/libhxnu_kernel.a ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o', 'DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o')
cmake = cmake.replace('--whole-archive ${CMAKE_CURRENT_BINARY_DIR}/libhxnu_kernel.a --no-whole-archive', '${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o')

cmake = cmake.replace('OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/libgboot.a ${CMAKE_CURRENT_BINARY_DIR}/gboot.o', 'OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/gboot.o')
cmake = cmake.replace('-o ${CMAKE_CURRENT_BINARY_DIR}/libgboot.a', '-o ${CMAKE_CURRENT_BINARY_DIR}/gboot.o')
cmake = cmake.replace('COMMAND cp ${CMAKE_CURRENT_BINARY_DIR}/libgboot.a ${CMAKE_CURRENT_BINARY_DIR}/gboot.o\n', '')
cmake = cmake.replace('DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/libgboot.a ${CMAKE_CURRENT_BINARY_DIR}/gboot.o', 'DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/gboot.o')
cmake = cmake.replace('--whole-archive ${CMAKE_CURRENT_BINARY_DIR}/libgboot.a --no-whole-archive', '${CMAKE_CURRENT_BINARY_DIR}/gboot.o')

with open('CMakeLists.txt', 'w') as f:
    f.write(cmake)

