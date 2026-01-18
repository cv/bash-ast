for i in {1..10}; do
    for j in {1..10}; do
        if (( i * j > 50 )); then
            break 2
        fi
        if (( (i + j) % 2 == 0 )); then
            continue
        fi
        echo "$i x $j"
    done
done

while :; do
    read -r cmd || break
    [[ $cmd == "skip" ]] && continue
    [[ $cmd == "quit" ]] && break
    eval "$cmd"
done
