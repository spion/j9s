
# j9s - the k9s for Jira

j9s is a terminal-based UI to interact with Jira, inspired by [k9s](https://k9scli.io/).
It allows you to quickly browse, search, and manage Jira issues from the command line.

## Features

- Browse Jira projects and issues in a terminal UI
- Same interface as k9s for object types:
  - The active context is the project
  - `:issues` - shows the entire project list. Edit an issue with `e`, view details with `Enter`.
    - Shortcuts for common things like changing labels, assignees, status, etc.
  - `:boards` -> Issues
  - Toggle board filtering by assignee with `Shift-a`
  - Quick search everywhere with `/`
  - `:epics` - view epics in the project, Enter to view issues in the epic
  - `:searches` - saved searches (Jira filters)
- Ideally, local caching for offline use and for performance improvement.
- Configurable via a YAML config file