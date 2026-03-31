-- Durable block indexing tables for marketplace-ready datasets
CREATE TABLE IF NOT EXISTS public.blocks (
    height BIGINT PRIMARY KEY,
    hash TEXT NOT NULL UNIQUE,
    parent_hash TEXT,
    block_timestamp TIMESTAMPTZ,
    miner TEXT,
    tx_count INTEGER NOT NULL DEFAULT 0,
    work_score NUMERIC(28,8),
    raw_header JSONB NOT NULL DEFAULT '{}'::jsonb,
    raw_block JSONB NOT NULL DEFAULT '{}'::jsonb,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_blocks_hash ON public.blocks (hash);
CREATE INDEX IF NOT EXISTS idx_blocks_parent_hash ON public.blocks (parent_hash);
CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON public.blocks (block_timestamp DESC);

CREATE TABLE IF NOT EXISTS public.block_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    block_height BIGINT NOT NULL REFERENCES public.blocks(height) ON DELETE CASCADE,
    tx_index INTEGER NOT NULL,
    tx_hash TEXT NOT NULL,
    tx_type TEXT NOT NULL,
    signer TEXT,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT block_transactions_height_index_unique UNIQUE (block_height, tx_index),
    CONSTRAINT block_transactions_hash_unique UNIQUE (tx_hash)
);

CREATE INDEX IF NOT EXISTS idx_block_transactions_height ON public.block_transactions (block_height);
CREATE INDEX IF NOT EXISTS idx_block_transactions_hash ON public.block_transactions (tx_hash);
CREATE INDEX IF NOT EXISTS idx_block_transactions_type ON public.block_transactions (tx_type);

CREATE TABLE IF NOT EXISTS public.marketplace_block_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    block_height BIGINT NOT NULL REFERENCES public.blocks(height) ON DELETE CASCADE,
    tx_hash TEXT NOT NULL,
    tx_index INTEGER NOT NULL,
    event_index INTEGER NOT NULL DEFAULT 0,
    event_type TEXT NOT NULL,
    problem_id TEXT,
    task_id UUID,
    order_id UUID,
    trade_id UUID,
    actor_wallet TEXT,
    amount NUMERIC(28,8),
    event_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT marketplace_block_events_unique UNIQUE (block_height, tx_index, event_index)
);

CREATE INDEX IF NOT EXISTS idx_marketplace_block_events_height ON public.marketplace_block_events (block_height DESC);
CREATE INDEX IF NOT EXISTS idx_marketplace_block_events_type ON public.marketplace_block_events (event_type);
CREATE INDEX IF NOT EXISTS idx_marketplace_block_events_problem ON public.marketplace_block_events (problem_id);
CREATE INDEX IF NOT EXISTS idx_marketplace_block_events_actor ON public.marketplace_block_events (actor_wallet);

CREATE TABLE IF NOT EXISTS public.dataset_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL,
    version TEXT NOT NULL,
    dataset_type TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    start_height BIGINT NOT NULL,
    end_height BIGINT NOT NULL,
    row_count BIGINT NOT NULL DEFAULT 0,
    manifest JSONB NOT NULL DEFAULT '{}'::jsonb,
    checksum TEXT,
    storage_path TEXT,
    status TEXT NOT NULL DEFAULT 'ready',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT dataset_snapshots_height_range CHECK (end_height >= start_height),
    CONSTRAINT dataset_snapshots_slug_version_unique UNIQUE (slug, version)
);

CREATE INDEX IF NOT EXISTS idx_dataset_snapshots_slug ON public.dataset_snapshots (slug, created_at DESC);

CREATE TABLE IF NOT EXISTS public.dataset_catalog (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,
    dataset_type TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    price NUMERIC(28,8) NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'BEANS',
    visibility TEXT NOT NULL DEFAULT 'public',
    latest_snapshot_id UUID REFERENCES public.dataset_snapshots(id) ON DELETE SET NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER update_dataset_snapshots_updated_at
    BEFORE UPDATE ON public.dataset_snapshots
    FOR EACH ROW
    EXECUTE FUNCTION public.update_updated_at();

CREATE TRIGGER update_dataset_catalog_updated_at
    BEFORE UPDATE ON public.dataset_catalog
    FOR EACH ROW
    EXECUTE FUNCTION public.update_updated_at();

ALTER TABLE public.blocks ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.block_transactions ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.marketplace_block_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.dataset_snapshots ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.dataset_catalog ENABLE ROW LEVEL SECURITY;

CREATE POLICY blocks_read ON public.blocks
    FOR SELECT USING (true);

CREATE POLICY block_transactions_read ON public.block_transactions
    FOR SELECT USING (true);

CREATE POLICY marketplace_block_events_read ON public.marketplace_block_events
    FOR SELECT USING (true);

CREATE POLICY dataset_snapshots_read ON public.dataset_snapshots
    FOR SELECT USING (status = 'ready');

CREATE POLICY dataset_catalog_read ON public.dataset_catalog
    FOR SELECT USING (visibility = 'public');

INSERT INTO public.dataset_catalog (slug, dataset_type, title, description, price, currency, visibility, metadata)
VALUES
    (
        'marketplace-events-by-block',
        'marketplace_events',
        'Marketplace Events By Block',
        'Confirmed marketplace events normalized per block for analytics and dataset delivery.',
        0,
        'BEANS',
        'public',
        '{"category":"marketplace","refresh_strategy":"per_confirmed_block"}'::jsonb
    ),
    (
        'problem-submissions-and-solutions',
        'problem_activity',
        'Problem Submissions And Solutions',
        'PoUW problem creation and solver submission activity extracted from on-chain marketplace transactions.',
        0,
        'BEANS',
        'public',
        '{"category":"research","refresh_strategy":"per_confirmed_block"}'::jsonb
    ),
    (
        'bounty-payout-history',
        'bounty_payouts',
        'Bounty Payout History',
        'Claimed bounty history suitable for reward analysis and auditing.',
        0,
        'BEANS',
        'public',
        '{"category":"rewards","refresh_strategy":"per_confirmed_block"}'::jsonb
    ),
    (
        'trading-and-liquidity-activity',
        'trading_activity',
        'Trading And Liquidity Activity',
        'Liquidity and trading-related events derived from normalized chain data.',
        0,
        'BEANS',
        'public',
        '{"category":"trading","refresh_strategy":"per_confirmed_block"}'::jsonb
    )
ON CONFLICT (slug) DO NOTHING;

ALTER TABLE public.sync_state
    ADD COLUMN IF NOT EXISTS last_finalized_height BIGINT NOT NULL DEFAULT 0;

ALTER TABLE public.sync_state
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now();
