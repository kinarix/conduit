-- Phase 23.1 — RBAC redesign: wipe legacy tenant data.
--
-- This migration is the "clean break" half of the RBAC redesign. It removes
-- all existing tenant data so the new schema (org_members,
-- org_role_assignments, global_role_assignments) starts empty. Migrations 028
-- through 031 then reshape the schema itself.
--
-- The operator must re-bootstrap (CONDUIT_BOOTSTRAP_ADMIN_EMAIL/_PASSWORD)
-- after these migrations run. See bootstrap.rs.

-- Truncate every table that has an FK chain rooted at orgs/users. CASCADE
-- across all dependent rows in one shot to avoid FK ordering pain.
TRUNCATE TABLE
    execution_history,
    parallel_join_state,
    process_events,
    timer_start_triggers,
    event_subscriptions,
    jobs,
    tasks,
    variables,
    executions,
    process_instances,
    process_layouts,
    process_definitions,
    decision_definitions,
    process_groups,
    secrets,
    org_auth_config,
    api_keys,
    user_roles,
    role_permissions,
    users,
    orgs
RESTART IDENTITY CASCADE;

-- Wipe custom roles (org-scoped). Global built-in role definitions are kept
-- for now; migration 031 will rewrite them with the new permission catalog.
DELETE FROM roles WHERE org_id IS NOT NULL;

-- The `_platform` / `conduit` system org no longer exists as a concept.
-- We dropped the row via TRUNCATE above; now drop the flag column.
ALTER TABLE orgs DROP COLUMN IF EXISTS is_system;
