#!/bin/bash
# Verify TCOL/HPL header in all new files
HEADER="TCOL / HPL (HXNU Public License)"
FAIL=0
for file in $(find . -type f \( -name "*.rs" -o -name "*.c" -o -name "*.cpp" -o -name "*.h" -o -name "*.tlscript" -o -name "CMakeLists.txt" \) -not -path "*/.agents/*" -not -path "*/build/*" -not -path "*/target/*" -not -path "*/.git/*" -not -path "*/vendor/*"); do
    if ! head -n 5 "$file" | grep -q "$HEADER"; then
        echo "Missing TCOL/HPL header: $file"
        FAIL=1
    fi
done
if [ $FAIL -eq 0 ]; then
    echo "All files contain TCOL/HPL header."
else
    echo "Licensing check failed."
    exit 1
fi
