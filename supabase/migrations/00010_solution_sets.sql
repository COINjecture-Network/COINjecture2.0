-- Verified solution-set data for downloadable blockchain solution corpora
CREATE TABLE IF NOT EXISTS public.solution_sets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    block_height BIGINT NOT NULL REFERENCES public.blocks(height) ON DELETE CASCADE,
    block_hash TEXT NOT NULL,
    problem_id TEXT,
    problem_type TEXT NOT NULL,
    solution_type TEXT NOT NULL,
    miner TEXT,
    work_score NUMERIC(28,8),
    solve_time_us BIGINT,
    verify_time_us BIGINT,
    time_asymmetry_ratio DOUBLE PRECISION,
    solution_quality DOUBLE PRECISION,
    complexity_weight DOUBLE PRECISION,
    energy_estimate_joules DOUBLE PRECISION,
    quality_band TEXT,
    raw_problem JSONB NOT NULL DEFAULT '{}'::jsonb,
    raw_solution JSONB NOT NULL DEFAULT '{}'::jsonb,
    raw_solution_reveal JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT solution_sets_block_height_unique UNIQUE (block_height)
);

CREATE INDEX IF NOT EXISTS idx_solution_sets_problem_type
    ON public.solution_sets (problem_type);
CREATE INDEX IF NOT EXISTS idx_solution_sets_solution_quality
    ON public.solution_sets (solution_quality DESC NULLS LAST);
CREATE INDEX IF NOT EXISTS idx_solution_sets_work_score
    ON public.solution_sets (work_score DESC NULLS LAST);
CREATE INDEX IF NOT EXISTS idx_solution_sets_quality_band
    ON public.solution_sets (quality_band);

ALTER TABLE public.solution_sets ENABLE ROW LEVEL SECURITY;

CREATE POLICY solution_sets_read ON public.solution_sets
    FOR SELECT USING (true);

INSERT INTO public.dataset_catalog (slug, dataset_type, title, description, price, currency, visibility, metadata)
VALUES (
    'verified-solution-sets',
    'solution_sets',
    'Verified Solution Sets',
    'Block-linked NP problem and solution bundles with quality, work, and timing metrics for sorting and benchmarking.',
    0,
    'BEANS',
    'public',
    '{"category":"ai-research","refresh_strategy":"per_confirmed_block","sortable_fields":["problem_type","solution_quality","work_score"]}'::jsonb
)
ON CONFLICT (slug) DO NOTHING;
