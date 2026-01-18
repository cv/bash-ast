coproc BC { bc -l; }

echo "scale=10; 4*a(1)" >&${BC[1]}
read -r pi <&${BC[0]}
echo "Pi = $pi"

echo "quit" >&${BC[1]}
wait $BC_PID

coproc FILTER {
    while read -r line; do
        [[ $line == *error* ]] && echo "$line"
    done
}

tail -f /var/log/syslog >&${FILTER[1]} &
