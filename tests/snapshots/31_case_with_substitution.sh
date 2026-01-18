case "$(uname -s)" in
    Linux*)
        OS=linux
        PKG_MGR=$(command -v apt || command -v yum)
        ;;
    Darwin*)
        OS=macos
        PKG_MGR=$(command -v brew)
        ;;
    CYGWIN*|MINGW*|MSYS*)
        OS=windows
        ;;
    *)
        OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
        ;;
esac
