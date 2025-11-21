#!/usr/bin/env python3
"""
Clear HuggingFace dataset data directory to prepare for schema-normalized uploads
"""

from huggingface_hub import HfApi, CommitOperationDelete
import sys

# HuggingFace configuration
HF_TOKEN = "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"
DATASET_NAME = "COINjecture/NP_Solutions"

def delete_data_directory():
    """Delete all files in the data/ directory"""

    api = HfApi()

    print(f"[*] Listing files in {DATASET_NAME}/data...")

    try:
        # List all files in the dataset
        files = api.list_repo_files(
            repo_id=DATASET_NAME,
            repo_type="dataset",
            token=HF_TOKEN
        )

        # Filter for data directory files
        data_files = [f for f in files if f.startswith("data/")]

        if not data_files:
            print("[+] data/ directory is already empty")
            return True

        print(f"[*] Found {len(data_files)} files to delete")
        for file_path in data_files:
            print(f"   - {file_path}")

        # Create delete operations for all files
        print(f"\n[*] Deleting {len(data_files)} files...")
        operations = [CommitOperationDelete(path_in_repo=file_path) for file_path in data_files]

        # Execute all delete operations in a single commit
        api.create_commit(
            repo_id=DATASET_NAME,
            repo_type="dataset",
            operations=operations,
            commit_message="Clear dataset for energy measurement deployment",
            token=HF_TOKEN
        )

        print(f"[+] Successfully deleted {len(data_files)} files from dataset")
        return True

    except Exception as e:
        print(f"[-] Failed to delete files: {e}")
        return False

if __name__ == "__main__":
    print("[*] Clearing HuggingFace dataset for schema normalization...")
    print(f"    Dataset: {DATASET_NAME}")
    print()

    success = delete_data_directory()

    if success:
        print("\n[+] Dataset cleared successfully!")
        print("    Ready for energy-enabled uploads")
        sys.exit(0)
    else:
        print("\n[-] Failed to clear dataset")
        sys.exit(1)
