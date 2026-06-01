# Recorded with VHS — https://github.com/charmbracelet/vhs
#   vhs menus.tape
# 'dockpit' must be on PATH. GIFs were recorded against a throwaway Docker
# daemon containing only demo containers (api/cache/db/web/worker).

Output "menus.gif"
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
Type "d"
Sleep 2500ms
Escape
Sleep 800ms
Type "c"
Sleep 2800ms
Escape
Sleep 1200ms
