{
    echo "Starting batch"
    cmd1
    cmd2
    echo "Finished batch"
} > batch.log 2>&1

{ read -r header; cat; } < data.csv > body.csv

{
    echo "User: $USER"
    echo "Date: $(date)"
    env | sort
} | {
    while read -r line; do
        printf "[LOG] %s\n" "$line"
    done
}
