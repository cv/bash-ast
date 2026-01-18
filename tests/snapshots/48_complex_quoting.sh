echo 'Single quotes: $VAR and `cmd` are literal'
echo "Double quotes: $VAR and $(cmd) are expanded"
echo $'ANSI-C: \t tab \n newline \x41'
echo $"Locale: translated string"

cmd="echo 'nested \"quotes\" here'"
eval "$cmd"

printf '%s\n' "Line with
actual newline"

grep "pattern with spaces" <<< "$var"
ssh host "echo \"remote: \$HOME\""
