-- Add 'ralphloop' to the run_reason CHECK constraint

-- 1. Add replacement column with wider CHECK that includes ralphloop
ALTER TABLE execution_processes
  ADD COLUMN run_reason_new TEXT NOT NULL DEFAULT 'codingagent'
    CHECK (run_reason_new IN ('setupscript',
                              'cleanupscript',
                              'codingagent',
                              'devserver',
                              'ralphloop'));

-- 2. Copy existing values
UPDATE execution_processes
  SET run_reason_new = run_reason;

-- 3. Drop any indexes on the old column
DROP INDEX IF EXISTS idx_execution_processes_run_reason;

-- 4. Remove the old column
ALTER TABLE execution_processes DROP COLUMN run_reason;

-- 5. Rename new column to canonical name
ALTER TABLE execution_processes
  RENAME COLUMN run_reason_new TO run_reason;

-- 6. Recreate index
CREATE INDEX idx_execution_processes_run_reason
  ON execution_processes(run_reason);
