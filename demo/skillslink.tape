Output "./assets/skillslink-demo.gif"

Set Shell "bash"
Set FontSize 24
Set Width 1280
Set Height 760
Set Framerate 30
Set Padding 28
Set Theme "Catppuccin Mocha"
Set CursorBlink false
Set TypingSpeed 15ms

Type "clear"
Enter
Sleep 250ms

Type "./target/release/skillslink --config ./.tmp_demo/skillslink.toml link --force"
Enter
Sleep 450ms

Type "ls -li ./.tmp_demo/master.md ./.tmp_demo/AGENTS.md ./.tmp_demo/CLAUDE.md"
Enter
Sleep 700ms

Type "./target/release/skillslink --config ./.tmp_demo/skillslink.toml verify"
Enter
Sleep 900ms

Type "./target/release/skillslink install-commit-guard --repo ./.tmp_demo/repo --force"
Enter
Sleep 500ms

Type "./.tmp_demo/repo/.git/hooks/commit-msg ./.tmp_demo/repo/.git/COMMIT_EDITMSG"
Enter
Sleep 250ms

Type "cat ./.tmp_demo/repo/.git/COMMIT_EDITMSG"
Enter
Sleep 800ms
