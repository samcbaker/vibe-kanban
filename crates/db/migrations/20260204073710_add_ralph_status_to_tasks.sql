-- Add ralph_status column to tasks table for tracking Ralph AI loop execution
-- Default value 'none' indicates no Ralph execution is active

ALTER TABLE tasks ADD COLUMN ralph_status TEXT NOT NULL DEFAULT 'none'
    CHECK (ralph_status IN ('none', 'planning', 'awaitingapproval', 'building', 'completed', 'failed'));
