-- When the current editor took the edit lock, so others can see how long they have been editing.
-- lock_heartbeat_at keeps moving while they work; this does not.
ALTER TABLE committee_pages ADD COLUMN lock_since timestamptz;
