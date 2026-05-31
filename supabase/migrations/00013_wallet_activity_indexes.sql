-- Speed up wallet activity lookups by signer / miner.
CREATE INDEX IF NOT EXISTS idx_block_transactions_signer
    ON public.block_transactions (signer)
    WHERE signer IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_blocks_miner
    ON public.blocks (miner)
    WHERE miner IS NOT NULL;
