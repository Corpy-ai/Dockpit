# Recorded with VHS — https://github.com/charmbracelet/vhs
#   vhs filter.tape
# 'dockpit' must be on PATH. GIFs were recorded against a throwaway Docker
# daemon containing only demo containers (api/cache/db/web/worker).

Output "filter.gif"
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
Tab@1600ms 6
Sleep 1500ms
