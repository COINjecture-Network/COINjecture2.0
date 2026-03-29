#!/usr/bin/env bash
# Links this repo to the hosted Supabase project. Run from your machine (not in CI without secrets).
#
# Usage:
#   export SUPABASE_ACCESS_TOKEN="sbp_..."   # https://supabase.com/dashboard/account/tokens
#   optional: export SUPABASE_DB_PASSWORD="..."  # skips DB password prompt on link
#   ./scripts/supabase-login-and-link.sh
#
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ -z "${SUPABASE_ACCESS_TOKEN:-}" ]]; then
  echo "Missing SUPABASE_ACCESS_TOKEN."
  echo "Create one at https://supabase.com/dashboard/account/tokens then:"
  echo "  export SUPABASE_ACCESS_TOKEN='sbp_...'"
  exit 1
fi

PROJECT_REF="${SUPABASE_PROJECT_REF:-xmpjbhuzahtqsaspqitz}"

echo "Logging in to Supabase CLI..."
npx supabase login --token "$SUPABASE_ACCESS_TOKEN"

echo "Linking project $PROJECT_REF..."
if [[ -n "${SUPABASE_DB_PASSWORD:-}" ]]; then
  export SUPABASE_DB_PASSWORD
fi
npx supabase link --project-ref "$PROJECT_REF"

echo "Done. Verify with: npx supabase migration list"
