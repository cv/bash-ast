cat <<EOF
Variables expand: $HOME
Commands run: $(date)
EOF

cat <<'LITERAL'
Variables don't expand: $HOME
Commands don't run: $(date)
LITERAL

cat <<-INDENTED
	Tabs are stripped
	From the beginning
	INDENTED

read -r -d '' JSON <<'END' || true
{
    "name": "test",
    "value": 42
}
END

echo "$JSON"
