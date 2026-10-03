# Security

Please report a vulnerability privately rather than in a public issue: through
[a private advisory](https://github.com/sachesi/gill/security/advisories/new) on GitHub,
or by mail to sachesi <xsachesi@pm.me>. Say what you found, how to reproduce it and
which version you ran; a fix is worked out with you before anything is published.

Only the latest release gets fixes.

gill runs what you type, so a command doing harm is not a vulnerability. What counts is
anything that makes fish or bash run code you did not type: a variable value, file name
or directory name that changes what fish executes when gill applies bash's state, or
another user on the same machine reading or changing the files in
`$XDG_RUNTIME_DIR/gill`.
