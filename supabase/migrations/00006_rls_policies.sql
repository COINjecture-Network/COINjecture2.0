-- Enable RLS on all tables
ALTER TABLE public.wallet_bindings ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.user_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.orders ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.trades ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.trading_pairs ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.pouw_tasks ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.task_assignments ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.reputation_events ENABLE ROW LEVEL SECURITY;

-- === WALLET BINDINGS ===
-- Users can read their own bindings
CREATE POLICY wallet_bindings_select ON public.wallet_bindings
    FOR SELECT USING (auth.uid() = user_id);

-- Users can insert their own bindings
CREATE POLICY wallet_bindings_insert ON public.wallet_bindings
    FOR INSERT WITH CHECK (auth.uid() = user_id);

-- Users can update their own bindings (e.g., set primary)
CREATE POLICY wallet_bindings_update ON public.wallet_bindings
    FOR UPDATE USING (auth.uid() = user_id);

-- Users can delete their own bindings
CREATE POLICY wallet_bindings_delete ON public.wallet_bindings
    FOR DELETE USING (auth.uid() = user_id);

-- === USER PROFILES ===
-- Anyone can read profiles
CREATE POLICY user_profiles_select ON public.user_profiles
    FOR SELECT USING (true);

-- Users can insert/update their own profile
CREATE POLICY user_profiles_insert ON public.user_profiles
    FOR INSERT WITH CHECK (auth.uid() = id);

CREATE POLICY user_profiles_update ON public.user_profiles
    FOR UPDATE USING (auth.uid() = id);

-- === TRADING PAIRS ===
-- Anyone can read active pairs
CREATE POLICY trading_pairs_select ON public.trading_pairs
    FOR SELECT USING (is_active = true);

-- === ORDERS ===
-- Users can read their own orders
CREATE POLICY orders_select_own ON public.orders
    FOR SELECT USING ((SELECT auth.uid()) = user_id);

-- Users can read all open orders (order book is public)
CREATE POLICY orders_select_open ON public.orders
    FOR SELECT USING (status = 'open');

-- Users can insert their own orders
CREATE POLICY orders_insert ON public.orders
    FOR INSERT WITH CHECK ((SELECT auth.uid()) = user_id);

-- Users can cancel their own pending/open orders
CREATE POLICY orders_update ON public.orders
    FOR UPDATE USING ((SELECT auth.uid()) = user_id);

-- === TRADES ===
-- Anyone can read finalized trades (public trade history)
CREATE POLICY trades_select_public ON public.trades
    FOR SELECT USING (is_finalized = true);

-- Users can read their own trades (including unfinalized)
CREATE POLICY trades_select_own ON public.trades
    FOR SELECT USING (
        buyer_wallet = (auth.jwt() -> 'app_metadata' ->> 'wallet_address')
        OR seller_wallet = (auth.jwt() -> 'app_metadata' ->> 'wallet_address')
    );

-- === POUW TASKS ===
-- Anyone can read open tasks
CREATE POLICY pouw_tasks_select_open ON public.pouw_tasks
    FOR SELECT USING (status IN ('open', 'completed'));

-- Users can read their own tasks (any status)
CREATE POLICY pouw_tasks_select_own ON public.pouw_tasks
    FOR SELECT USING ((SELECT auth.uid()) = submitter_user_id);

-- Users can create tasks
CREATE POLICY pouw_tasks_insert ON public.pouw_tasks
    FOR INSERT WITH CHECK ((SELECT auth.uid()) = submitter_user_id);

-- Users can update their own draft/open tasks
CREATE POLICY pouw_tasks_update ON public.pouw_tasks
    FOR UPDATE USING (
        (SELECT auth.uid()) = submitter_user_id
        AND status IN ('draft', 'open')
    );

-- === TASK ASSIGNMENTS ===
-- Task submitters and assignees can read assignments
CREATE POLICY task_assignments_select ON public.task_assignments
    FOR SELECT USING (
        (SELECT auth.uid()) = miner_user_id
        OR (SELECT auth.uid()) IN (
            SELECT submitter_user_id FROM public.pouw_tasks WHERE id = task_id
        )
    );

-- Users can create assignments for open tasks
CREATE POLICY task_assignments_insert ON public.task_assignments
    FOR INSERT WITH CHECK ((SELECT auth.uid()) = miner_user_id);

-- Miners can update their own assignments (submit solutions)
CREATE POLICY task_assignments_update ON public.task_assignments
    FOR UPDATE USING ((SELECT auth.uid()) = miner_user_id);

-- === REPUTATION EVENTS ===
-- Anyone can read reputation events (transparency)
CREATE POLICY reputation_events_select ON public.reputation_events
    FOR SELECT USING (true);

-- Only service role can insert reputation events
-- (No policy for authenticated users — inserts happen via Edge Functions / service role)
