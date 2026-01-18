(
    cd /var/log
    cat syslog | grep error | tail -100
) | mail -s "Errors" admin@example.com

result=$(
    find . -type f -name "*.log" |
    xargs grep -l "FATAL" |
    wc -l
)

( cmd1 & cmd2 & cmd3 & wait ) 2>&1 | tee output.log
