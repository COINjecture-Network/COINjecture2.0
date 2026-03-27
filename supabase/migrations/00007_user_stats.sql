-- Function to get user statistics
-- Called via PostgREST: GET /rest/v1/rpc/get_user_stats
CREATE OR REPLACE FUNCTION public.get_user_stats()
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
DECLARE
    result jsonb;
BEGIN
    SELECT jsonb_build_object(
        'total_users', (SELECT count(*) FROM auth.users),
        'wallet_only_users', (
            SELECT count(DISTINCT wb.user_id) FROM public.wallet_bindings wb
            LEFT JOIN auth.users u ON u.id = wb.user_id
            WHERE u.email LIKE '%@coinjecture.beans'
        ),
        'email_only_users', (
            SELECT count(*) FROM auth.users u
            WHERE u.email NOT LIKE '%@coinjecture.beans'
            AND u.id NOT IN (SELECT user_id FROM public.wallet_bindings)
        ),
        'email_with_wallet_users', (
            SELECT count(DISTINCT wb.user_id) FROM public.wallet_bindings wb
            LEFT JOIN auth.users u ON u.id = wb.user_id
            WHERE u.email NOT LIKE '%@coinjecture.beans'
        ),
        'signups_last_24h', (
            SELECT count(*) FROM auth.users
            WHERE created_at > now() - interval '24 hours'
        ),
        'signups_last_7d', (
            SELECT count(*) FROM auth.users
            WHERE created_at > now() - interval '7 days'
        ),
        'total_wallet_bindings', (
            SELECT count(*) FROM public.wallet_bindings
        ),
        'active_wallets', (
            SELECT count(DISTINCT wallet_address) FROM public.wallet_bindings
        )
    ) INTO result;

    RETURN result;
END;
$$;

-- Only service role can call this
REVOKE EXECUTE ON FUNCTION public.get_user_stats FROM anon, authenticated;
