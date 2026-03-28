-- Trading pairs
CREATE TABLE public.trading_pairs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    base_token TEXT NOT NULL,     -- e.g., 'BEANS'
    quote_token TEXT NOT NULL,    -- e.g., 'USDC'
    is_active BOOLEAN NOT NULL DEFAULT true,
    min_order_size NUMERIC(28,8) NOT NULL DEFAULT 0.00000001,
    tick_size NUMERIC(28,8) NOT NULL DEFAULT 0.00000001,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT unique_pair UNIQUE (base_token, quote_token)
);

-- Order types and statuses
CREATE TYPE order_side AS ENUM ('buy', 'sell');
CREATE TYPE order_type AS ENUM ('limit', 'market', 'stop_limit');
CREATE TYPE order_status AS ENUM ('pending', 'open', 'partially_filled', 'filled', 'cancelled', 'expired', 'rejected');

-- Orders table
CREATE TABLE public.orders (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES auth.users(id),
    wallet_address TEXT NOT NULL,
    pair_id UUID NOT NULL REFERENCES public.trading_pairs(id),
    side order_side NOT NULL,
    type order_type NOT NULL,
    status order_status NOT NULL DEFAULT 'pending',
    price NUMERIC(28,8),            -- NULL for market orders
    quantity NUMERIC(28,8) NOT NULL,
    filled_quantity NUMERIC(28,8) NOT NULL DEFAULT 0,
    remaining_quantity NUMERIC(28,8) GENERATED ALWAYS AS (quantity - filled_quantity) STORED,
    stop_price NUMERIC(28,8),       -- for stop_limit orders
    time_in_force TEXT DEFAULT 'GTC', -- GTC, IOC, FOK
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    on_chain_tx_hash TEXT           -- links to blockchain tx
);

-- Critical index for order book queries
CREATE INDEX idx_open_orders_book ON public.orders (pair_id, side, price)
    WHERE status = 'open';
CREATE INDEX idx_user_orders ON public.orders (user_id, created_at DESC);
CREATE INDEX idx_wallet_orders ON public.orders (wallet_address);

-- Trades table (partitioned by month from day one)
CREATE TABLE public.trades (
    id UUID NOT NULL DEFAULT uuid_generate_v4(),
    pair_id UUID NOT NULL REFERENCES public.trading_pairs(id),
    buy_order_id UUID NOT NULL,
    sell_order_id UUID NOT NULL,
    buyer_wallet TEXT NOT NULL,
    seller_wallet TEXT NOT NULL,
    price NUMERIC(28,8) NOT NULL,
    quantity NUMERIC(28,8) NOT NULL,
    fee_amount NUMERIC(28,8) NOT NULL DEFAULT 0,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    is_finalized BOOLEAN NOT NULL DEFAULT false,
    on_chain_tx_hash TEXT,
    block_height BIGINT,

    PRIMARY KEY (id, executed_at)
) PARTITION BY RANGE (executed_at);

-- Create initial partitions (monthly)
CREATE TABLE public.trades_2026_01 PARTITION OF public.trades
    FOR VALUES FROM ('2026-01-01') TO ('2026-02-01');
CREATE TABLE public.trades_2026_02 PARTITION OF public.trades
    FOR VALUES FROM ('2026-02-01') TO ('2026-03-01');
CREATE TABLE public.trades_2026_03 PARTITION OF public.trades
    FOR VALUES FROM ('2026-03-01') TO ('2026-04-01');
CREATE TABLE public.trades_2026_04 PARTITION OF public.trades
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');
CREATE TABLE public.trades_2026_05 PARTITION OF public.trades
    FOR VALUES FROM ('2026-05-01') TO ('2026-06-01');
CREATE TABLE public.trades_2026_06 PARTITION OF public.trades
    FOR VALUES FROM ('2026-06-01') TO ('2026-07-01');

-- Trade indexes
CREATE INDEX idx_trades_pair_time ON public.trades (pair_id, executed_at DESC);
CREATE INDEX idx_trades_buyer ON public.trades (buyer_wallet, executed_at DESC);
CREATE INDEX idx_trades_seller ON public.trades (seller_wallet, executed_at DESC);

-- Trigger for orders updated_at
CREATE TRIGGER update_orders_updated_at
    BEFORE UPDATE ON public.orders
    FOR EACH ROW
    EXECUTE FUNCTION public.update_updated_at();
