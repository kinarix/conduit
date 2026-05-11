-- Tracks whether the first-time setup wizard has been completed for an org.
-- Defaults TRUE so existing orgs (created before this migration) are not
-- forced back through the wizard. bootstrap.rs sets it to FALSE on first boot.
ALTER TABLE orgs ADD COLUMN setup_completed BOOLEAN NOT NULL DEFAULT TRUE;
