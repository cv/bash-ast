shopt -s nullglob globstar

for f in **/*.txt; do
    echo "Doc: $f"
done

for f in *.jpg *.jpeg *.png *.gif; do
    echo "Image: $f"
done

case "$file" in
    *.tar|*.tar.gz|*.tgz|*.tar.bz2)
        tar xf "$file" ;;
    *.zip)
        unzip "$file" ;;
esac

[[ $name == [A-Z]* ]]
