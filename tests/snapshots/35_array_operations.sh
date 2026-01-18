files=()
declare -A counts

while IFS= read -r -d '' file; do
    files+=("$file")
    ext="${file##*.}"
    (( counts[$ext]++ ))
done < <(find . -type f -print0)

echo "Total: ${#files[@]}"
for ext in "${!counts[@]}"; do
    printf "%s: %d\n" "$ext" "${counts[$ext]}"
done | sort -t: -k2 -rn
