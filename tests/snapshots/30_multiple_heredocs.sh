cat <<'SCRIPT' > script.sh
#!/bin/bash
echo "Hello"
SCRIPT

cat <<-INDENTED
	This heredoc
	uses tabs for
	indentation
INDENTED

mysql <<SQL
SELECT * FROM users
WHERE active = 1;
SQL
