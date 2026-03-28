-- Reputation event types
CREATE TYPE reputation_event_type AS ENUM (
    'task_completed',       -- miner completed a task
    'task_rejected',        -- miner's solution was rejected
    'task_submitted',       -- submitter created a task
    'trade_completed',      -- user completed a trade
    'dispute_won',          -- user won a dispute
    'dispute_lost',         -- user lost a dispute
    'peer_uptime_bonus',    -- node operator uptime reward
    'manual_adjustment'     -- admin adjustment
);

-- Append-only reputation events
CREATE TABLE public.reputation_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES auth.users(id),
    wallet_address TEXT NOT NULL,
    event_type reputation_event_type NOT NULL,
    score_delta NUMERIC(10,4) NOT NULL,
    reference_id UUID,           -- FK to task, trade, etc.
    reference_type TEXT,         -- 'task', 'trade', 'dispute'
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_rep_events_user ON public.reputation_events (user_id, created_at DESC);
CREATE INDEX idx_rep_events_wallet ON public.reputation_events (wallet_address);

-- Materialized view for current reputation scores
CREATE MATERIALIZED VIEW public.reputation_scores AS
SELECT
    user_id,
    wallet_address,
    SUM(score_delta) as total_score,
    COUNT(*) FILTER (WHERE event_type = 'task_completed') as tasks_completed,
    COUNT(*) FILTER (WHERE event_type = 'trade_completed') as trades_completed,
    COUNT(*) FILTER (WHERE event_type = 'task_rejected') as tasks_rejected,
    MAX(created_at) as last_activity
FROM public.reputation_events
GROUP BY user_id, wallet_address;

CREATE UNIQUE INDEX idx_rep_scores_user ON public.reputation_scores (user_id);
CREATE INDEX idx_rep_scores_wallet ON public.reputation_scores (wallet_address);

-- Function to refresh reputation (call after events)
CREATE OR REPLACE FUNCTION public.refresh_reputation_scores()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY public.reputation_scores;
END;
$$ LANGUAGE plpgsql;
