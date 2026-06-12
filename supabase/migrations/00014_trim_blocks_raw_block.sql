-- Shrink indexed blocks: keep coinbase for wallet rewards, drop duplicated full block JSON.
UPDATE public.blocks
SET raw_block = jsonb_build_object('coinbase', raw_block->'coinbase')
WHERE raw_block IS NOT NULL
  AND raw_block != '{}'::jsonb
  AND raw_block ? 'transactions';

COMMENT ON COLUMN public.blocks.raw_block IS
    'Minimal block slice for wallet UI (coinbase reward). Full txs live in block_transactions.';
