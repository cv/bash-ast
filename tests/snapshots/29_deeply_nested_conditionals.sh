if [[ -f "$1" ]]; then
    if [[ -r "$1" ]]; then
        if [[ -s "$1" ]]; then
            if grep -q "pattern" "$1"; then
                echo "Found pattern in non-empty readable file"
            else
                echo "Pattern not found"
            fi
        else
            echo "File is empty"
        fi
    else
        echo "File not readable"
    fi
else
    echo "Not a file"
fi
