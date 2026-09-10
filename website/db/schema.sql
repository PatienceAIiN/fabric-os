CREATE TABLE IF NOT EXISTS download_events (
  id BIGSERIAL PRIMARY KEY,
  platform VARCHAR(32) NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS download_events_created_at_idx
  ON download_events (created_at DESC);

CREATE TABLE IF NOT EXISTS consent_events (
  id BIGSERIAL PRIMARY KEY,
  policy_version VARCHAR(40) NOT NULL,
  necessary BOOLEAN NOT NULL DEFAULT TRUE,
  analytics BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS consent_events_created_at_idx
  ON consent_events (created_at DESC);
