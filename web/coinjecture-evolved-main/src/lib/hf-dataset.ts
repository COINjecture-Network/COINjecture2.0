/** Hub repo id nodes upload to when `HF_DATASET_NAME` / `--hf-dataset-name` is set. */
export const HF_DATASET_HUB_ID = "COINjecture/NP-Solutions";

/** Public Hugging Face dataset page (override with `VITE_HF_DATASET_URL` at build time). */
export function hfDatasetPageUrl(): string {
  const fromEnv = (import.meta.env.VITE_HF_DATASET_URL as string | undefined)?.trim();
  if (fromEnv) return fromEnv;
  return `https://huggingface.co/datasets/${HF_DATASET_HUB_ID}`;
}
