# Bug: $EDITOR not receiving key events

## Symptom

When launching `$EDITOR` from the issue editor view, the editor window appears (after the Redraw fix) but does not receive any key input. The terminal is not being handed over properly.

## Current code

`src/ui/views/issue_editor.rs:395-423` — `launch_editor()`:

```rust
let _ = crossterm::terminal::disable_raw_mode();
let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);

let status = std::process::Command::new(&editor).arg(&tmp_path).status();

let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
let _ = crossterm::terminal::enable_raw_mode();
```

## Likely causes to investigate

1. **EventHandler thread still consuming stdin** — `src/event.rs` spawns a background thread that reads crossterm events in a loop. Even after disabling raw mode and leaving the alternate screen, that thread is still calling `crossterm::event::read()` or `crossterm::event::poll()`, racing with the child process for stdin bytes. The editor gets no input because the event thread swallows it.

2. **stdin not inherited by Command** — `std::process::Command::status()` should inherit stdin/stdout/stderr by default, but worth verifying no `.stdin(Stdio::null())` or similar is set.

3. **Raw mode not fully disabled** — the `disable_raw_mode()` result is silently dropped. If it fails, the editor gets raw-mode input which could look like "no events" depending on the editor.

## Suggested fix direction

The most likely culprit is #1. The event handler thread needs to be paused or stopped before launching the editor. Options:

- **Pause/resume on EventHandler**: Add methods to pause the event loop thread (e.g. via an `AtomicBool` flag that makes the thread skip reads), and resume after editor returns.
- **Drop and recreate EventHandler**: Stop the event handler before launching editor, recreate after. This is simpler but has more overhead.
- **Channel-based suspend**: Send a suspend message to the event thread, wait for ack, then launch editor.

The fix should happen at the App level since it owns the EventHandler. This may require `launch_editor` to return a new `ViewAction` variant (e.g. `ViewAction::LaunchEditor(callback)`) so the App can pause events, run the editor, then resume — or the current `ViewAction::Redraw` approach could be extended.

## Files to examine

- `src/event.rs` — EventHandler implementation, how it reads events
- `src/app.rs:64-93` — main loop, owns EventHandler
- `src/ui/views/issue_editor.rs:395-423` — launch_editor()
- `src/ui/view.rs` — ViewAction enum
