-- Phase 23.1 — RBAC redesign: users become global identities.
--
-- Multi-org membership is now an explicit relation (see migration 029).
-- A user no longer "belongs to" a single org via users.org_id; instead they
-- are members of zero or more orgs via org_members. Email becomes globally
-- unique because the user identity is global.

ALTER TABLE users DROP COLUMN IF EXISTS org_id;

-- Drop the old (org_id, email) UNIQUE constraint name in case it lingers as
-- a dangling object (DROP COLUMN above removed it implicitly, but be explicit).
ALTER TABLE users DROP CONSTRAINT IF EXISTS uq_users_org_email;

-- Drop the old per-org email index.
DROP INDEX IF EXISTS idx_users_org_id;

-- Global email uniqueness. Case-insensitive to match real-world login flows.
CREATE UNIQUE INDEX users_email_lower_key ON users (LOWER(email));
