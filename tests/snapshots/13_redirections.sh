echo hello > out.txt
cat < in.txt
echo append >> log.txt
cmd 2>&1
cmd &> all.txt
exec 3>&-
cmd >| force.txt
