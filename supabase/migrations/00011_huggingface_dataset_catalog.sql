-- Marketplace catalog entry pointing at the public NP solutions dataset on the Hub.
-- (HF API tokens never belong in SQL — set HF_TOKEN / HF_DATASET_NAME in node env only.)
INSERT INTO public.dataset_catalog (slug, dataset_type, title, description, price, currency, visibility, metadata)
VALUES (
    'huggingface-np-solutions',
    'huggingface_mirror',
    'Hugging Face — NP-Solutions (solution sets)',
    'Hub mirror for verified per-block solution sets: work, timing, quality, energy, and raw PoUW payloads.',
    0,
    'BEANS',
    'public',
    '{"category":"huggingface","huggingface_repo":"COINjecture/NP-Solutions","refresh_strategy":"hub_managed","aligns_with_catalog_slug":"verified-solution-sets"}'::jsonb
)
ON CONFLICT (slug) DO UPDATE SET
    title = EXCLUDED.title,
    description = EXCLUDED.description,
    metadata = EXCLUDED.metadata,
    updated_at = now();
