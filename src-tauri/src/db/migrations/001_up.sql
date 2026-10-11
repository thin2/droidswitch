CREATE TABLE provider_connections (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),
    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
    base_url TEXT NOT NULL CHECK(length(base_url) > 0),
    protocol TEXT NOT NULL CHECK(protocol IN ('anthropic', 'openai', 'generic-chat-completion-api')),
    credential_mode TEXT NOT NULL CHECK(credential_mode IN ('environment', 'direct')),
    credential_value TEXT NOT NULL,
    model_list_url TEXT,
    extra_headers TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(extra_headers) AND json_type(extra_headers) = 'object'),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
CREATE TABLE model_entries (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),
    provider_id TEXT NOT NULL REFERENCES provider_connections(id) ON DELETE RESTRICT,
    api_model_id TEXT NOT NULL CHECK(length(api_model_id) > 0),
    droid_selector_id TEXT,
    display_name TEXT NOT NULL,
    manufacturer TEXT,
    protocol_override TEXT CHECK(protocol_override IN ('anthropic', 'openai', 'generic-chat-completion-api')),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    max_output_tokens INTEGER CHECK(max_output_tokens > 0),
    image_input INTEGER NOT NULL DEFAULT 0 CHECK(image_input IN (0, 1)),
    extra_args TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(extra_args) AND json_type(extra_args) = 'object'),
    last_test TEXT CHECK(last_test IS NULL OR json_valid(last_test)),
    UNIQUE(provider_id, api_model_id)
) STRICT;
CREATE TABLE apply_snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    target_path TEXT NOT NULL,
    backup_path TEXT NOT NULL,
    fingerprint TEXT NOT NULL CHECK(json_valid(fingerprint)),
    applied_state TEXT NOT NULL CHECK(json_valid(applied_state)),
    created_at INTEGER NOT NULL
) STRICT;
CREATE TABLE drafts (
    id TEXT PRIMARY KEY NOT NULL,
    target_path TEXT NOT NULL UNIQUE,
    base_snapshot_id TEXT REFERENCES apply_snapshots(id) ON DELETE RESTRICT,
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision >= 0),
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    updated_at INTEGER NOT NULL
) STRICT;
-- Historical attribution is a snapshot, intentionally independent of mutable provider/model rows.
CREATE TABLE usage_events (
    id TEXT PRIMARY KEY NOT NULL,
    source_file TEXT NOT NULL,
    event_key TEXT NOT NULL,
    session_id TEXT NOT NULL,
    occurred_at INTEGER NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('official', 'api', 'unknown')),
    provider_id_at_event TEXT,
    model_id_at_event TEXT,
    input_tokens INTEGER CHECK(input_tokens >= 0),
    cache_read_tokens INTEGER CHECK(cache_read_tokens >= 0),
    cache_write_tokens INTEGER CHECK(cache_write_tokens >= 0),
    output_tokens INTEGER CHECK(output_tokens >= 0),
    official_credits REAL CHECK(official_credits >= 0),
    estimated_cost TEXT,
    currency TEXT,
    price_id_at_event TEXT,
    UNIQUE(source_file, event_key)
) STRICT;
CREATE INDEX usage_events_time ON usage_events(occurred_at);
CREATE TABLE sync_cursors (
    source_file TEXT PRIMARY KEY NOT NULL,
    file_identity TEXT NOT NULL,
    byte_offset INTEGER NOT NULL CHECK(byte_offset >= 0),
    state TEXT NOT NULL CHECK(json_valid(state)),
    updated_at INTEGER NOT NULL
) STRICT;
-- Decimal strings preserve manual prices exactly; arithmetic validation belongs to the pricing layer.
CREATE TABLE model_prices (
    id TEXT PRIMARY KEY NOT NULL,
    model_id TEXT NOT NULL,
    currency TEXT NOT NULL CHECK(length(currency) > 0),
    input_per_million TEXT,
    cache_read_per_million TEXT,
    cache_write_per_million TEXT,
    output_per_million TEXT,
    source TEXT NOT NULL,
    effective_at INTEGER NOT NULL,
    UNIQUE(model_id, currency, effective_at)
) STRICT;
