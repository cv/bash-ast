cmd 2>&1 1>&3 3>&-
{ cmd1; cmd2; } > out.txt 2>&1
(cmd1; cmd2) < in.txt > out.txt
