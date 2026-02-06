-- Add ralph_enabled column to tasks table
-- Enables Ralph Mode (AI-driven task execution) for individual tasks
-- SQLite uses INTEGER for booleans (0 = false, 1 = true)
ALTER TABLE tasks ADD COLUMN ralph_enabled INTEGER NOT NULL DEFAULT 0;
