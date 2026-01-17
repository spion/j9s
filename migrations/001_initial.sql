-- Initial schema for j9s local cache
-- This file is kept for reference; the schema is embedded in the binary

-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY
);

-- Cached issues
CREATE TABLE IF NOT EXISTS issues (
    key TEXT PRIMARY KEY,
    project TEXT NOT NULL,
    summary TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL,
    issue_type TEXT NOT NULL,
    assignee TEXT,
    reporter TEXT,
    priority TEXT,
    labels TEXT,  -- JSON array
    created TEXT NOT NULL,
    updated TEXT NOT NULL,
    cached_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_issues_project ON issues(project);
CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
CREATE INDEX IF NOT EXISTS idx_issues_assignee ON issues(assignee);

-- Cached boards
CREATE TABLE IF NOT EXISTS boards (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    board_type TEXT NOT NULL,
    cached_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Cached sprints
CREATE TABLE IF NOT EXISTS sprints (
    id INTEGER PRIMARY KEY,
    board_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    state TEXT NOT NULL,
    start_date TEXT,
    end_date TEXT,
    cached_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (board_id) REFERENCES boards(id)
);

CREATE INDEX IF NOT EXISTS idx_sprints_board ON sprints(board_id);

-- Insert initial schema version
INSERT OR IGNORE INTO schema_version (version) VALUES (1);
