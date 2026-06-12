-- Run in Supabase Dashboard → SQL Editor when the project hits exceed_db_size_quota.
-- Shrinks indexed blocks (full block JSON was duplicated in block_transactions).

-- 1) Trim oversized block rows (migration 00014)
UPDATE public.blocks
SET raw_block = jsonb_build_object('coinbase', raw_block->'coinbase')
WHERE raw_block IS NOT NULL
  AND raw_block != '{}'::jsonb
  AND raw_block ? 'transactions';

-- 2) Reclaim disk (may take a minute on large tables)
VACUUM FULL public.blocks;
VACUUM FULL public.block_transactions;
VACUUM FULL public.marketplace_block_events;

-- 3) Optional: reset indexer to re-sync from current chain tip after a genesis restart
-- DELETE FROM public.indexer_sync_state;

-- After this: restore Supabase plan/spend cap if still blocked, then restart api-server so INDEXER resumes.
