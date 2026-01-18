echo "$(echo "$(echo "deep")")"

result=$(cat <<< "$(date +%Y)-$(hostname)-$(whoami)")

version=$(grep -oP 'version=\K[0-9.]+' <<< "$(cat config.txt)")

files=$(find . -name "$(basename "$(pwd)")*.txt")

eval "$(ssh-agent -s)"

export PATH="$(dirname "$(which python)")/../lib:$PATH"
