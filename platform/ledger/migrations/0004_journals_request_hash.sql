-- Durable idempotency: the journals row is the source of truth for replay.
-- The request hash lets a replay verify the payload even after the
-- idempotency_keys cache row has expired or been pruned. Rows inserted before
-- this migration have NULL and replay without hash verification (dev-only
-- data; no production history exists).
ALTER TABLE journals ADD COLUMN request_hash TEXT;