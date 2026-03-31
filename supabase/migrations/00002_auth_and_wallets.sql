-- Wallet bindings table
-- Links Supabase auth users to COINjecture wallet addresses
CREATE TABLE public.wallet_bindings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    wallet_address TEXT NOT NULL,
    chain_id TEXT NOT NULL DEFAULT 'coinjecture:testnet',
    is_primary BOOLEAN NOT NULL DEFAULT false,
    bound_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    siwb_signature TEXT, -- original SIWB signature as proof

    CONSTRAINT unique_wallet_per_chain UNIQUE (wallet_address, chain_id)
);

-- Ensure only one primary wallet per user per chain
CREATE UNIQUE INDEX idx_one_primary_per_user_chain
    ON public.wallet_bindings (user_id, chain_id)
    WHERE is_primary = true;

-- Fast lookup by wallet address
CREATE INDEX idx_wallet_address ON public.wallet_bindings (wallet_address);
CREATE INDEX idx_user_wallets ON public.wallet_bindings (user_id);

-- User profiles (extended info beyond auth.users)
CREATE TABLE public.user_profiles (
    id UUID PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
    display_name TEXT,
    avatar_url TEXT,
    bio TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Custom access token hook: injects wallet_address into JWT
CREATE OR REPLACE FUNCTION public.custom_access_token_hook(event jsonb)
RETURNS jsonb
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
    claims jsonb;
    wallet_addr text;
BEGIN
    claims := event->'claims';

    -- Look up the user's primary wallet address
    SELECT wallet_address INTO wallet_addr
    FROM public.wallet_bindings
    WHERE user_id = (event->'claims'->>'sub')::uuid
      AND is_primary = true
    LIMIT 1;

    IF wallet_addr IS NOT NULL THEN
        claims := jsonb_set(claims, '{wallet_address}', to_jsonb(wallet_addr));
    END IF;

    -- Set app_metadata for RLS policies
    claims := jsonb_set(claims, '{app_metadata,wallet_address}', to_jsonb(COALESCE(wallet_addr, '')));

    RETURN jsonb_set(event, '{claims}', claims);
END;
$$;

-- Grant execute to supabase_auth_admin (required for hooks)
GRANT EXECUTE ON FUNCTION public.custom_access_token_hook TO supabase_auth_admin;
REVOKE EXECUTE ON FUNCTION public.custom_access_token_hook FROM authenticated, anon, public;

-- Trigger to update updated_at
CREATE OR REPLACE FUNCTION public.update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_user_profiles_updated_at
    BEFORE UPDATE ON public.user_profiles
    FOR EACH ROW
    EXECUTE FUNCTION public.update_updated_at();
