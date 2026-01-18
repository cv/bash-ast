for server in web{01..05}.example.com; do
    ssh "$server" 'uptime' &
done
wait

for ext in {jpg,png,gif}; do
    find . -name "*.$ext" -exec convert {} {}.webp \;
done

mkdir -p project/{src,lib,test}/{main,util}
