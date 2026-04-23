-- Link marketplace "verified-solution-sets" to the NP-Solutions Hub dataset (same scientific lineage).
UPDATE public.dataset_catalog
SET
    metadata = metadata
        || jsonb_build_object(
            'huggingface_repo', 'COINjecture/NP-Solutions',
            'huggingface_catalog_slug', 'huggingface-np-solutions'
        ),
    updated_at = now()
WHERE slug = 'verified-solution-sets';
