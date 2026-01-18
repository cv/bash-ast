case "$1" in
    start|begin)
        echo "Starting..."
        ;;
    stop|end)
        echo "Stopping..."
        ;;
    *)
        echo "Unknown: $1"
        ;;
esac
