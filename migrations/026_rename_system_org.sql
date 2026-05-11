-- Renames the hidden system org from slug=`_platform`/name=`Platform` to
-- slug=`conduit`/name=`Conduit`. The underscore-prefix convention is dropped
-- in favour of a single named reservation: the slug `conduit` is now reserved
-- (alongside any `is_system = TRUE` row).
--
-- Idempotent: skips the rename if a `conduit` row already exists (e.g.,
-- fresh installs that hit migration 026 right after a future-rewritten 025).

UPDATE orgs
SET    slug = 'conduit', name = 'Conduit'
WHERE  slug = '_platform'
  AND  NOT EXISTS (SELECT 1 FROM orgs WHERE slug = 'conduit');
