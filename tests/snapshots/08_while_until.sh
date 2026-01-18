while read line; do
    echo "$line"
done

until test -f /tmp/done; do
    sleep 1
done
