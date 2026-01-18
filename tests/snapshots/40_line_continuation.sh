long_command \
    --option1=value1 \
    --option2=value2 \
    --flag \
    arg1 arg2 arg3

result=$(echo "hello" | \
    tr 'a-z' 'A-Z' | \
    sed 's/HELLO/WORLD/')

[[ $var == "value" && \
   $other == "test" || \
   -f $file ]] && echo "match"
