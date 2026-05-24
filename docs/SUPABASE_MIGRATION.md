# Supabase migration (`zmaodanzkfabhvmjiccr`)

New hosted project: **https://zmaodanzkfabhvmjiccr.supabase.co**

Previous default link ref in scripts was `xmpjbhuzahtqsaspqitz` — update all runtime secrets to the new project.

## 1. Cursor MCP

Project config: [`.cursor/mcp.json`](../.cursor/mcp.json)

```json
{
  "mcpServers": {
    "supabase": {
      "url": "https://mcp.supabase.com/mcp?project_ref=zmaodanzkfabhvmjiccr"
    }
  }
}
```

After adding or changing this file:

1. Reload Cursor (or **Settings → MCP →** enable **supabase**).
2. Complete browser OAuth when prompted.

**Two MCP servers may appear:**

| Server | How to use |
|--------|------------|
| **supabase** (plugin, from `.cursor/mcp.json`) | Pass `"project_id": "zmaodanzkfabhvmjiccr"` on every tool call |
| **user-supabase** (legacy user-level) | May still point at the old project until removed — disable it if `get_project_url` returns `xmpjbhuzahtqsaspqitz` |

Verify the new project: `get_project_url` with `project_id` **zmaodanzkfabhvmjiccr** → `https://zmaodanzkfabhvmjiccr.supabase.co`.

## 2. Agent skills (optional)

```bash
npx skills add supabase/agent-skills
```

Uses [Supabase agent skills](https://github.com/supabase/agent-skills) for Postgres/RLS/migration guidance in the IDE.

## 3. CLI link + push migrations

```bash
export SUPABASE_ACCESS_TOKEN="sbp_..."   # Dashboard → Account → Access Tokens
export SUPABASE_PROJECT_REF=zmaodanzkfabhvmjiccr
# optional: export SUPABASE_DB_PASSWORD="..."  # project DB password

./scripts/supabase-login-and-link.sh
npx supabase db push
```

Migrations live in [`supabase/migrations/`](../supabase/migrations/) (`00001`–`00012`).

## 4. Runtime env (production)

Set on **api-server** and **web** (never commit real keys):

| Variable | Where |
|----------|--------|
| `SUPABASE_URL` | `https://zmaodanzkfabhvmjiccr.supabase.co` |
| `SUPABASE_ANON_KEY` | Dashboard → Project Settings → API |
| `SUPABASE_JWT_SECRET` | Same (JWT secret) |
| `SUPABASE_SERVICE_ROLE_KEY` | Same (server only) |
| `VITE_SUPABASE_URL` | Web build (`web/coinjecture-evolved-main`) |
| `VITE_SUPABASE_ANON_KEY` | Web build |

Templates: [`api-server/.env.example`](../api-server/.env.example), [`.env.production.example`](../.env.production.example), [`scripts/deployment/hostinger-vps/api.env.example`](../scripts/deployment/hostinger-vps/api.env.example).

**Hostinger / VPS:** update `/opt/coinjecture` (or your deploy path) `.env` and restart `api-server` + rebuild web if env is baked at build time.

## 5. Auth redirects

In Supabase Dashboard → **Authentication → URL configuration**, set:

- Site URL: your production app (e.g. `https://coinjecture.com`)
- Redirect URLs: same origins as [`supabase/config.toml`](../supabase/config.toml) `additional_redirect_urls` (local + CloudFront + production).

## 6. Data migration (optional)

If you need rows from the old project:

1. `pg_dump` / Supabase backup from old project, or
2. Re-index from chain via api-server indexer after empty schema + migrations.

Indexer tables: see migrations `00008`–`00012` (sync state, chain events, solution sets, Hugging Face catalog).

## 7. Verify

```bash
# MCP or CLI (project zmaodanzkfabhvmjiccr)
npx supabase migration list
```

In Cursor, MCP `list_tables` with `project_id: zmaodanzkfabhvmjiccr` should show **21** `public` tables (including trade partitions) and `sync_state` with 1 row.

**Schema status (2026-05-24):** All repo migrations `00001`–`00012` applied to `zmaodanzkfabhvmjiccr` via MCP (`extensions` through `solution_sets_hf_np_solutions`). Indexer will backfill `blocks` / `solution_sets` from chain height 0 when api-server points here.

**Note:** Trade month partitions (`trades_2026_01`–`06`) inherit RLS from parent `trades` in Postgres but Supabase advisors may flag them — same as the prior project; enable partition RLS only if you add matching policies.
