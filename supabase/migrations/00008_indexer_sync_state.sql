-- Indexer sync state — tracks which block height has been indexed
CREATE TABLE IF NOT EXISTS public.sync_state (
    id TEXT PRIMARY KEY DEFAULT 'main',
    last_indexed_height BIGINT NOT NULL DEFAULT 0,
    last_indexed_hash TEXT NOT NULL DEFAULT '',
    last_sync_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Insert initial state
INSERT INTO public.sync_state (id, last_indexed_height, last_indexed_hash)
VALUES ('main', 0, '')
ON CONFLICT (id) DO NOTHING;

-- Trigger for updated_at
CREATE TRIGGER update_sync_state_updated_at
    BEFORE UPDATE ON public.sync_state
    FOR EACH ROW
    EXECUTE FUNCTION public.update_updated_at();

-- RLS: anyone can read, only service role can write
ALTER TABLE public.sync_state ENABLE ROW LEVEL SECURITY;

CREATE POLICY sync_state_read ON public.sync_state
    FOR SELECT USING (true);
