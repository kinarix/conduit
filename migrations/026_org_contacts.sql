-- Org contact + description fields. Captured at create time and
-- editable later through the admin → General page. All nullable —
-- existing orgs (and orgs created by callers who skip these fields)
-- simply have NULL values. No data backfill required.
ALTER TABLE orgs
    ADD COLUMN admin_email   TEXT,
    ADD COLUMN admin_name    TEXT,
    ADD COLUMN support_email TEXT,
    ADD COLUMN description   TEXT;
