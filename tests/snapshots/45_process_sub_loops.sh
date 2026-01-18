while IFS= read -r line; do
    echo "Processing: $line"
done < <(find . -type f -newer "$marker" -print)

paste <(cut -f1 file1.tsv) <(cut -f2 file2.tsv) > merged.tsv

diff -u \
    <(ssh server1 'cat /etc/config') \
    <(ssh server2 'cat /etc/config')

tee >(gzip > backup.gz) >(wc -l > count.txt) < input.txt > output.txt
