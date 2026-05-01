-- Phase 16: HTTP connector config snapshot.
--
-- Holds the serialized HttpConfig (method, auth, transforms, retry policy) for
-- http_task jobs at the moment they are enqueued. Snapshotting on the job — vs.
-- looking it up from the deployment graph at fire time — means redeploying the
-- definition cannot mutate in-flight calls. Secret *names* are persisted here;
-- secret *values* are resolved from the secrets table at fire time, so rotation
-- works without redeploy.
ALTER TABLE jobs ADD COLUMN config JSONB;
