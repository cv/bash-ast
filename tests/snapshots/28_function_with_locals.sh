process_file() {
    local file="$1"
    local -i count=0
    local line
    while IFS= read -r line; do
        (( count++ ))
        echo "[$count] $line"
    done < "$file"
    return $count
}
