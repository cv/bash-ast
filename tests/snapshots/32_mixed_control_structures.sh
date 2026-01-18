for config in /etc/*.conf; do
    if [[ -r "$config" ]]; then
        while IFS='=' read -r key value; do
            case "$key" in
                \#*|"") continue ;;
                timeout) (( value > 0 )) && TIMEOUT=$value ;;
                *) export "$key=$value" ;;
            esac
        done < "$config"
    fi
done
