ALTER TABLE usage_events ADD COLUMN cost_complete INTEGER NOT NULL DEFAULT 0 CHECK(cost_complete IN (0,1));
ALTER TABLE usage_events ADD COLUMN price_snapshot TEXT;
-- Keep imported request identities and legacy cumulative semantics distinct.
UPDATE usage_events SET api_model_id=model_id_at_event,
  granularity=CASE WHEN event_key LIKE 'cumulative:%' THEN 'legacy_cumulative' ELSE 'request' END
WHERE api_model_id IS NULL AND session_ref IS NULL AND endpoint_id IS NULL AND model_selector IS NULL;

CREATE INDEX usage_events_order ON usage_events(occurred_at DESC,id DESC);
