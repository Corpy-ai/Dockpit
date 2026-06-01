# Recorded with VHS — https://github.com/charmbracelet/vhs
#   vhs search.tape
# 'dockpit' must be on PATH. GIFs were recorded against a throwaway Docker
# daemon containing only demo containers (api/cache/db/web/worker).

Output "search.gif"
Set FontSize 15
Set Width 1000
Set Height 600
Set Padding 12
Set Theme "Dracula"
Hide
Type "dockpit"
Enter
Sleep 3s
Show
Down@800ms 1
Up@800ms 1
Sleep 3s
Type "/"
Sleep 700ms
Type "error"
Sleep 900ms
Enter
Sleep 1500ms
Type "n"
Sleep 1300ms
Type "n"
Sleep 1300ms
Type "N"
Sleep 1500ms
Escape
Sleep 1000ms
