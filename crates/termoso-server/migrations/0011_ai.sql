-- Per-account opt-in for AI command suggestions (server-wide availability
-- is a deployment setting; this is the user saying yes).
ALTER TABLE users ADD COLUMN ai_enabled boolean NOT NULL DEFAULT false;
