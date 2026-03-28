-- Problem class enum
CREATE TYPE problem_class AS ENUM (
    'subset_sum', 'boolean_sat', 'tsp',
    'graph_coloring', 'knapsack', 'vertex_cover', 'custom'
);

CREATE TYPE task_status AS ENUM (
    'draft', 'open', 'assigned', 'submitted',
    'verifying', 'completed', 'expired', 'disputed', 'cancelled'
);

-- PoUW Tasks
CREATE TABLE public.pouw_tasks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    submitter_user_id UUID NOT NULL REFERENCES auth.users(id),
    submitter_wallet TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    problem_class problem_class NOT NULL,
    problem_data JSONB NOT NULL,          -- serialized problem instance
    bounty_amount NUMERIC(28,8) NOT NULL,
    bounty_token TEXT NOT NULL DEFAULT 'BEANS',
    min_work_score NUMERIC(10,4) NOT NULL DEFAULT 1.0,
    min_reputation NUMERIC(10,4) NOT NULL DEFAULT 0.0,
    max_assignments INT NOT NULL DEFAULT 1,
    status task_status NOT NULL DEFAULT 'draft',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deadline TIMESTAMPTZ NOT NULL,
    on_chain_escrow_tx TEXT,              -- tx hash of bounty escrow

    CONSTRAINT positive_bounty CHECK (bounty_amount > 0),
    CONSTRAINT future_deadline CHECK (deadline > created_at)
);

-- Task assignments (who is working on what)
CREATE TABLE public.task_assignments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_id UUID NOT NULL REFERENCES public.pouw_tasks(id),
    miner_user_id UUID NOT NULL REFERENCES auth.users(id),
    miner_wallet TEXT NOT NULL,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    submitted_at TIMESTAMPTZ,
    solution_data JSONB,                  -- serialized solution
    work_score NUMERIC(10,4),
    verification_status TEXT DEFAULT 'pending', -- pending, verified, rejected
    on_chain_tx_hash TEXT,

    CONSTRAINT unique_assignment UNIQUE (task_id, miner_user_id)
);

-- Indexes
CREATE INDEX idx_tasks_status ON public.pouw_tasks (status, deadline);
CREATE INDEX idx_tasks_class ON public.pouw_tasks (problem_class) WHERE status = 'open';
CREATE INDEX idx_tasks_submitter ON public.pouw_tasks (submitter_user_id);
CREATE INDEX idx_assignments_miner ON public.task_assignments (miner_user_id);
CREATE INDEX idx_assignments_task ON public.task_assignments (task_id);

-- Trigger
CREATE TRIGGER update_pouw_tasks_updated_at
    BEFORE UPDATE ON public.pouw_tasks
    FOR EACH ROW
    EXECUTE FUNCTION public.update_updated_at();
