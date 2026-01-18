#!/bin/bash
set -euo pipefail
IFS=$'\n\t'

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly LOG_FILE="${SCRIPT_DIR}/script.log"

declare -A CONFIG
PROCESSED=()

log() {
    local level="$1"; shift
    printf '[%s] [%s] %s\n' "$(date -Iseconds)" "$level" "$*" | tee -a "$LOG_FILE"
}

cleanup() {
    local exit_code=$?
    log INFO "Cleaning up (exit code: $exit_code)"
    rm -f "${TMPFILES[@]:-}"
    return $exit_code
}

trap cleanup EXIT
trap 'log ERROR "Interrupted"; exit 130' INT TERM

load_config() {
    local config_file="${1:?Config file required}"
    [[ -f "$config_file" ]] || { log ERROR "Config not found: $config_file"; return 1; }
    
    while IFS='=' read -r key value; do
        [[ $key =~ ^[[:space:]]*# ]] && continue
        [[ -z "$key" ]] && continue
        CONFIG["${key// /}"]="${value//\"/}"
    done < "$config_file"
}

process_files() {
    local file
    
    while IFS= read -r -d '' file; do
        if [[ -r "$file" ]] && (( $(wc -l < "$file") > 0 )); then
            case "${file##*.}" in
                txt) process_txt "$file" ;;
                log) process_log "$file" ;;
                *)   log WARN "Unknown type: $file" ;;
            esac
            PROCESSED+=("$file")
        fi
    done < <(find "${1:-.}" -type f \( -name "*.txt" -o -name "*.log" \) -print0)
    
    log INFO "Processed ${#PROCESSED[@]} files"
}

main() {
    log INFO "Starting script with args: $*"
    
    load_config "${CONFIG_FILE:-config.ini}" || exit 1
    
    for dir in "${@:-$PWD}"; do
        [[ -d "$dir" ]] && process_files "$dir"
    done
    
    {
        echo "Summary"
        echo "======="
        printf '%s\n' "${PROCESSED[@]}"
    } | mail -s "Processing complete" "${CONFIG[email]:-admin@localhost}"
}

main "$@"
