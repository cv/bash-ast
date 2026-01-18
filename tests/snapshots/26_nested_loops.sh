for i in 1 2 3; do
    for j in a b c; do
        while read line; do
            echo "$i $j: $line"
        done < "file_${i}_${j}.txt"
    done
done
