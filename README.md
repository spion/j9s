
# j9s - the k9s for Jira

j9s is a terminal-based UI to interact with Jira, inspired by [k9s](https://k9scli.io/).
It allows you to quickly browse, search, and manage Jira issues from the command line.

## Features

- Browse Jira projects and issues in a terminal UI
- Same interface as k9s for object types:
  - The active context is the project
  - `:issues` - shows the entire project list.
    - [ ] Edit an issue with `e`,
      - [ ] uses $EDITOR for issue description
    - [ ] Read comments, add comments
    - [x] view issue details with `Enter`.
    - [ ] Shortcuts for common edits like changing labels, assignees, transition, etc.
  - [x] `:boards` -> Issues
    - [x] swimlane (column) mode for boards
    - [x] move issues between columns with `Shift-Left` and `Shift-Right`
  - [ ] create new issues
  - [ ] Toggle board filtering by quick filters
  - [ ] Quick search everywhere with `/`
  - `:epics` - view epics in the project, Enter to view issues in the epic
  - `:searches` - saved searches (Jira filters)
- [ ] Ideally, local caching for offline use and for performance improvement.
- [x] Configurable via a YAML config file