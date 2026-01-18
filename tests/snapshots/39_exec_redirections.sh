exec 3>&1
exec 4>&2
exec 1>stdout.log
exec 2>stderr.log

echo "This goes to stdout.log"
echo "This goes to stderr.log" >&2

exec 1>&3 3>&-
exec 2>&4 4>&-

exec 5<> /tmp/fifo
echo "data" >&5
read -u5 response
exec 5>&-
