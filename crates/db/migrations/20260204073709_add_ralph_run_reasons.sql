-- Add RalphPlan and RalphBuild to execution_processes.run_reason CHECK constraint
-- SQLite requires table recreation to modify CHECK constraints

-- End auto-transaction to allow PRAGMA to take effect
-- https://github.com/launchbadge/sqlx/issues/2085#issuecomment-1499859906
COMMIT;

PRAGMA foreign_keys = OFF;

BEGIN TRANSACTION;

-- Create new table with updated CHECK constraint
CREATE TABLE execution_processes_new (
    id              BLOB PRIMARY KEY,
    session_id      BLOB NOT NULL,
    run_reason      TEXT NOT NULL DEFAULT 'setupscript'
                       CHECK (run_reason IN ('setupscript','codingagent','devserver','cleanupscript','ralphplan','ralphbuild')),
    executor_action TEXT NOT NULL DEFAULT '{}',
    status          TEXT NOT NULL DEFAULT 'running'
                       CHECK (status IN ('running','completed','failed','killed')),
    exit_code       INTEGER,
    dropped         INTEGER NOT NULL DEFAULT 0,
    started_at      TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    completed_at    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- Copy data from old table
INSERT INTO execution_processes_new
SELECT * FROM execution_processes;

-- Drop old table
DROP TABLE execution_processes;

-- Rename new table
ALTER TABLE execution_processes_new RENAME TO execution_processes;

-- Recreate all indexes from original migration
CREATE INDEX idx_execution_processes_session_id ON execution_processes(session_id);
CREATE INDEX idx_execution_processes_status ON execution_processes(status);
CREATE INDEX idx_execution_processes_run_reason ON execution_processes(run_reason);

-- Composite indexes for Task::find_by_project_id_with_attempt_status query optimization
CREATE INDEX idx_execution_processes_session_status_run_reason
ON execution_processes (session_id, status, run_reason);

CREATE INDEX idx_execution_processes_session_run_reason_created
ON execution_processes (session_id, run_reason, created_at DESC);

-- Verify foreign key constraints
PRAGMA foreign_key_check;

COMMIT;

PRAGMA foreign_keys = ON;

-- Start empty transaction for sqlx to close gracefully
BEGIN TRANSACTION;
