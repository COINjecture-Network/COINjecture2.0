#!/usr/bin/env python3
"""
COINjecture NP_Solutions_v5 Dataset Setup Script
Run: python setup_v5_dataset.py
"""

import requests
import json
import sys

HF_TOKEN = "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"
DATASET_NAME = "NP_Solutions_v5"
ORGANIZATION = "COINjecture"

# Output to both console and file
output_file = open("setup_output.txt", "w", encoding="utf-8")

def log(msg):
    print(msg)
    output_file.write(msg + "\n")
    output_file.flush()

def main():
    log("=" * 60)
    log("COINjecture v5 Dataset Setup")
    log("=" * 60)
    
    headers = {
        "Authorization": f"Bearer {HF_TOKEN}",
        "Content-Type": "application/json"
    }
    
    # Step 1: Check token validity
    log("\nStep 1: Verifying token...")
    try:
        response = requests.get(
            "https://huggingface.co/api/whoami-v2",
            headers=headers,
            timeout=30
        )
        if response.status_code == 200:
            user_info = response.json()
            log(f"   Token valid!")
            log(f"   User: {user_info.get('name', 'Unknown')}")
            
            orgs = [org.get('name', '') for org in user_info.get('orgs', [])]
            log(f"   Organizations: {orgs}")
            
            if ORGANIZATION not in orgs and ORGANIZATION != user_info.get('name'):
                log(f"\n   Warning: You may not have access to {ORGANIZATION}")
                log(f"   Creating under your personal account instead...")
                repo_id = f"{user_info.get('name')}/{DATASET_NAME}"
            else:
                repo_id = f"{ORGANIZATION}/{DATASET_NAME}"
        else:
            log(f"   Token invalid! Status: {response.status_code}")
            log(f"   Response: {response.text}")
            output_file.close()
            sys.exit(1)
    except Exception as e:
        log(f"   Error checking token: {e}")
        output_file.close()
        sys.exit(1)
    
    # Step 2: Create dataset
    log(f"\nStep 2: Creating dataset '{repo_id}'...")
    try:
        create_payload = {
            "type": "dataset",
            "name": DATASET_NAME,
            "private": False
        }
        
        if "/" in repo_id and repo_id.split("/")[0] != user_info.get('name'):
            create_payload["organization"] = ORGANIZATION
        
        response = requests.post(
            "https://huggingface.co/api/repos/create",
            headers=headers,
            json=create_payload,
            timeout=30
        )
        
        if response.status_code == 200:
            log(f"   Dataset created successfully!")
            log(f"   URL: https://huggingface.co/datasets/{repo_id}")
        elif response.status_code == 409:
            log(f"   Dataset already exists (this is OK)")
            log(f"   URL: https://huggingface.co/datasets/{repo_id}")
        else:
            log(f"   Failed to create dataset!")
            log(f"   Status: {response.status_code}")
            log(f"   Response: {response.text}")
    except Exception as e:
        log(f"   Error creating dataset: {e}")
        log("   Continuing anyway...")
    
    # Step 3: Upload README using huggingface_hub commit API
    log(f"\nStep 3: Uploading README...")
    try:
        with open("NP_Solutions_v5_README.md", "r", encoding="utf-8") as f:
            readme_content = f.read()
        
        # Use commit endpoint
        import base64
        encoded_content = base64.b64encode(readme_content.encode('utf-8')).decode('utf-8')
        
        commit_url = f"https://huggingface.co/api/datasets/{repo_id}/commit/main"
        
        commit_payload = {
            "operations": [
                {
                    "key": "file",
                    "value": {
                        "path": "README.md",
                        "encoding": "base64",
                        "content": encoded_content
                    }
                }
            ],
            "summary": "Add v5 README"
        }
        
        # Try simpler upload first
        upload_headers = {"Authorization": f"Bearer {HF_TOKEN}"}
        
        files = {'file': ('README.md', readme_content.encode('utf-8'), 'text/markdown')}
        upload_url = f"https://huggingface.co/api/datasets/{repo_id}/upload/main/README.md"
        
        response = requests.post(
            upload_url,
            headers=upload_headers,
            files=files,
            timeout=60
        )
        
        if response.status_code in [200, 201]:
            log(f"   README uploaded!")
        else:
            log(f"   README upload status: {response.status_code}")
            log(f"   Response: {response.text[:200] if response.text else 'empty'}")
            log(f"   You may need to add README manually via web interface")
    except FileNotFoundError:
        log(f"   NP_Solutions_v5_README.md not found")
    except Exception as e:
        log(f"   Error uploading README: {e}")
    
    # Summary
    log("\n" + "=" * 60)
    log("SETUP COMPLETE!")
    log("=" * 60)
    log(f"\nDataset URL: https://huggingface.co/datasets/{repo_id}")
    log(f"\nNext steps:")
    log(f"   1. Run the v5 deployment script:")
    log(f'      .\\deploy-v5-network.ps1 -HfToken "{HF_TOKEN}"')
    log(f"\n   2. Or run a single node:")
    log(f'      cargo run --release -- --mine --hf-token "{HF_TOKEN}" --hf-dataset-name "{repo_id}" --verbose')
    log("")
    
    output_file.close()

if __name__ == "__main__":
    main()

