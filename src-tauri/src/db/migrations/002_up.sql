-- Endpoint identity: the only direct evidence for provider attribution.
-- Normalized (scheme + host + path, no query) and stored once per distinct endpoint.
CREATE TABLE usage_endpoints (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),
    endpoint TEXT NOT NULL UNIQUE CHECK(length(endpoint) > 0),
    label TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

-- Secret-free history of applied model configuration. Written only after a
-- successful apply, so a failed apply produces no effective mapping.
CREATE TABLE usage_model_identities (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),
    config_version TEXT NOT NULL CHECK(length(config_version) > 0),
    effective_from INTEGER NOT NULL,
    effective_to INTEGER,
    droid_selector_id TEXT NOT NULL CHECK(length(droid_selector_id) > 0),
    api_model_id TEXT NOT NULL CHECK(length(api_model_id) > 0),
    internal_model_id TEXT,
    provider_id TEXT,
    provider_name TEXT NOT NULL,
    display_name TEXT NOT NULL,
    endpoint_id TEXT NOT NULL REFERENCES usage_endpoints(id) ON DELETE RESTRICT,
    UNIQUE(config_version, droid_selector_id)
) STRICT;
CREATE INDEX usage_model_identities_lookup ON usage_model_identities(droid_selector_id, effective_from DESC);

-- Session ledger. `own_*` is the session's own cumulative usage (exclusive of
-- child sessions); `inclusive_*` is Droid's rollup and is never summed.
-- A session without `snapshot_at` has no authoritative total.
CREATE TABLE usage_sessions (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),
    session_uuid TEXT NOT NULL CHECK(length(session_uuid) > 0),
    project_key TEXT NOT NULL,
    first_activity INTEGER,
    last_activity INTEGER,
    transcript_messages INTEGER NOT NULL DEFAULT 0 CHECK(transcript_messages >= 0),
    snapshot_at INTEGER,
    own_input_tokens INTEGER CHECK(own_input_tokens >= 0),
    own_cache_read_tokens INTEGER CHECK(own_cache_read_tokens >= 0),
    own_cache_write_tokens INTEGER CHECK(own_cache_write_tokens >= 0),
    own_output_tokens INTEGER CHECK(own_output_tokens >= 0),
    own_thinking_tokens INTEGER CHECK(own_thinking_tokens >= 0),
    own_credits REAL CHECK(own_credits >= 0),
    inclusive_input_tokens INTEGER CHECK(inclusive_input_tokens >= 0),
    inclusive_cache_read_tokens INTEGER CHECK(inclusive_cache_read_tokens >= 0),
    inclusive_cache_write_tokens INTEGER CHECK(inclusive_cache_write_tokens >= 0),
    inclusive_output_tokens INTEGER CHECK(inclusive_output_tokens >= 0),
    inclusive_thinking_tokens INTEGER CHECK(inclusive_thinking_tokens >= 0),
    child_sessions INTEGER NOT NULL DEFAULT 0 CHECK(child_sessions >= 0),
    updated_at INTEGER NOT NULL,
    UNIQUE(session_uuid, project_key)
) STRICT;

-- Detail columns. `granularity` separates a real request from a session-level
-- observation so a session total is never presented as a request count.
ALTER TABLE usage_events ADD COLUMN granularity TEXT NOT NULL DEFAULT 'request';
ALTER TABLE usage_events ADD COLUMN time_confidence TEXT NOT NULL DEFAULT 'event';
ALTER TABLE usage_events ADD COLUMN session_ref TEXT REFERENCES usage_sessions(id) ON DELETE SET NULL;
ALTER TABLE usage_events ADD COLUMN endpoint_id TEXT;
ALTER TABLE usage_events ADD COLUMN model_selector TEXT;
ALTER TABLE usage_events ADD COLUMN api_model_id TEXT;
ALTER TABLE usage_events ADD COLUMN thinking_tokens INTEGER CHECK(thinking_tokens >= 0);
ALTER TABLE usage_events ADD COLUMN request_id TEXT;
ALTER TABLE usage_events ADD COLUMN attempt INTEGER CHECK(attempt IS NULL OR attempt >= 1);
ALTER TABLE usage_events ADD COLUMN interval_start INTEGER;
ALTER TABLE usage_events ADD COLUMN interval_end INTEGER;
ALTER TABLE usage_events ADD COLUMN evidence TEXT NOT NULL DEFAULT 'unknown';
-- The same cost as `estimated_cost`, in units of 1e-12, so sums stay exact in SQL.
ALTER TABLE usage_events ADD COLUMN cost_units INTEGER;
CREATE INDEX usage_events_endpoint ON usage_events(endpoint_id, occurred_at);
CREATE INDEX usage_events_session ON usage_events(session_ref, occurred_at);
CREATE INDEX usage_events_granularity ON usage_events(granularity, occurred_at);

-- Pricing is per provider + API model + currency + effective time. The empty
-- provider means an explicit user-authored generic fallback, never a guess.
CREATE TABLE usage_prices (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),
    provider_id TEXT NOT NULL DEFAULT '',
    api_model_id TEXT NOT NULL CHECK(length(api_model_id) > 0),
    currency TEXT NOT NULL CHECK(length(currency) = 3),
    input_per_million TEXT,
    cache_read_per_million TEXT,
    cache_write_per_million TEXT,
    output_per_million TEXT,
    thinking_per_million TEXT,
    source TEXT NOT NULL,
    effective_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE(provider_id, api_model_id, currency, effective_at)
) STRICT;
CREATE INDEX usage_prices_lookup ON usage_prices(api_model_id, effective_at DESC);

-- Existing manual prices were model-only; they remain valid as generic fallbacks.
INSERT INTO usage_prices(id,provider_id,api_model_id,currency,input_per_million,cache_read_per_million,cache_write_per_million,output_per_million,source,effective_at,created_at)
SELECT id,'',model_id,currency,input_per_million,cache_read_per_million,cache_write_per_million,output_per_million,source,effective_at,effective_at FROM model_prices;
DROP TABLE model_prices;
