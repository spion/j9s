# j9s Development Notes

## Code Style

- Rust formatting: 100 columns, 2-space indentation (see `rustfmt.toml`)
- Run `cargo fmt` before committing

## Project Structure

- `src/app.rs` - Main app state and event loop
- `src/event.rs` - Event types and channel setup
- `src/config.rs` - XDG-compliant config loading
- `src/ui/` - Ratatui views
- `src/jira/` - Gouqi wrapper and types
- `src/db/` - SQLite caching (not yet integrated)

## Architecture

Uses k9s-style navigation:
- View stack with push/pop semantics
- `:` commands replace root view
- `Enter` pushes detail views
- `Escape` pops back

Message-passing for async:
- UI loop owns all state
- Async tasks send results via channel
- No shared mutable state across async boundaries

## Environment

- `J9S_JIRA_TOKEN` - Jira API token (required)
- Config at `~/.config/j9s/config.yaml`
