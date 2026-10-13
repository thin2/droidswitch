-- Restore the version 1 shape: usage_events without detail columns and
-- model_prices without provider scoping.
ALTER TABLE usage_events RENAME TO usage_events_v2;
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
INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,provider_id_at_event,model_id_at_event,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,official_credits,estimated_cost,currency,price_id_at_event)
SELECT id,source_file,event_key,session_id,occurred_at,source,provider_id_at_event,model_id_at_event,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,official_credits,estimated_cost,currency,price_id_at_event FROM usage_events_v2;
DROP TABLE usage_events_v2;
CREATE INDEX usage_events_time ON usage_events(occurred_at);

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
INSERT INTO model_prices(id,model_id,currency,input_per_million,cache_read_per_million,cache_write_per_million,output_per_million,source,effective_at)
SELECT id,api_model_id,currency,input_per_million,cache_read_per_million,cache_write_per_million,output_per_million,source,effective_at FROM usage_prices;
DROP TABLE usage_prices;

DROP TABLE usage_sessions;
DROP TABLE usage_model_identities;
DROP TABLE usage_endpoints;
