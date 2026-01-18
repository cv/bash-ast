cleanup() {
    rm -f "$TMPFILE"
    echo "Cleaned up"
}

trap cleanup EXIT
trap 'echo "Interrupted"; exit 130' INT TERM
trap '' HUP

TMPFILE=$(mktemp)
echo "Working..." > "$TMPFILE"
