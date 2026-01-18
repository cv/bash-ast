cat input.txt 2>/dev/null |
    grep -v '^#' |
    sort -u |
    tee intermediate.txt |
    awk '{print NR": "$0}' > output.txt 2>&1
