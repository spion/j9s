# j9s Demo - Jira TUI Client
# Run with: vhs demo.tape

# Configuration
Output demo.mp4
Set FontSize 16
Set Width 1200
Set Height 700
Set Theme "Catppuccin Mocha"
Set TypingSpeed 80ms

# Start the application
Type "./target/release/j9s"
Enter
Sleep 3s

# Show the default issue list view
Sleep 1s

# Open command palette and go to boards
Type ":"
Sleep 500ms
Type "boards"
Sleep 500ms
Enter
Sleep 2s

# Open the first board
Enter
Sleep 2s

# Toggle swimlane/kanban mode
Type "s"
Sleep 2s

# Navigate with vim keys
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "k"
Sleep 500ms

# Open filter bar and select assignee
Type "f"
Sleep 2s
Type "j"
Sleep 1s
Enter
Sleep 2s

# Cycle through assignees 3 times
PageDown
Sleep 1s
PageDown
Sleep 1s
PageDown
Sleep 1s

# Use search within board view
Type "/"
Sleep 500ms
Type "test"
Sleep 1s
Escape
Sleep 1s

# Go back to root
Escape
Sleep 1s
Escape
Sleep 1s

# Switch to issues view
Type ":"
Sleep 500ms
Type "issues"
Sleep 500ms
Enter
Sleep 2s

# Use search
Type "/"
Sleep 500ms
Type "test"
Sleep 1s
Enter
Sleep 1s

# Quit
Type ":"
Sleep 500ms
Type "quit"
Enter
Sleep 500ms
