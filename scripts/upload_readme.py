"""Upload README to HuggingFace v5 dataset"""
from huggingface_hub import HfApi

api = HfApi()

# Upload the README
api.upload_file(
    path_or_fileobj="v5_dataset_readme.md",
    path_in_repo="README.md",
    repo_id="COINjecture/v5",
    repo_type="dataset",
    token="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"
)

print("✅ README uploaded to https://huggingface.co/datasets/COINjecture/v5")

