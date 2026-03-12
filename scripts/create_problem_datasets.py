#!/usr/bin/env python3
"""
Create HuggingFace Problem-Type Datasets for COINjecture Network B
Institutional-grade dataset initialization with proper metadata and documentation
"""

from huggingface_hub import HfApi, DatasetCardData, DatasetCard
import sys

# HuggingFace configuration
HF_TOKEN = "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"
DATASET_PREFIX = "COINjecture"

# Problem type configurations
PROBLEM_TYPES = {
    "SAT": {
        "full_name": "Boolean Satisfiability Problem",
        "description": "SAT problem instances and solutions from COINjecture Network B blockchain mining",
        "schema_description": """
### Schema

Each record contains:
- `problem_id`: Unique identifier for the problem instance
- `problem_type`: "SAT"
- `problem_data`: SAT problem specification
  - `variables`: Number of boolean variables
  - `clauses`: List of clauses, each with list of literals
- `solution_data`: SAT solution (variable assignments)
  - `assignments`: Boolean assignments for each variable
- `problem_complexity`: Difficulty weight of the problem
- `bounty`: Token reward for solving (in smallest unit)
- `time_asymmetry`: Ratio of solve time to verify time
- `space_asymmetry`: Ratio of solve memory to verify memory
- `solve_energy_joules`: Energy consumed during solution
- `verify_energy_joules`: Energy consumed during verification
- `total_energy_joules`: Total energy consumed
- `energy_asymmetry`: Ratio of solve energy to verify energy
- `work_score`: Computed work score based on asymmetries
- `block_height`: Blockchain height where solution was accepted
- `timestamp`: Unix timestamp of solution
- `status`: Problem status (Mined, Validated, etc.)
- `energy_measurement_method`: Method used for energy measurement
- `submission_mode`: How problem was submitted (mining, public, private)
""",
        "tags": ["sat", "boolean-satisfiability", "np-complete", "blockchain", "proof-of-work"],
    },
    "TSP": {
        "full_name": "Traveling Salesman Problem",
        "description": "TSP problem instances and solutions from COINjecture Network B blockchain mining",
        "schema_description": """
### Schema

Each record contains:
- `problem_id`: Unique identifier for the problem instance
- `problem_type`: "TSP"
- `problem_data`: TSP problem specification
  - `cities`: Number of cities
  - `distances`: Distance matrix between cities
- `solution_data`: TSP solution (tour)
  - `tour`: Ordered list of city indices representing the tour
- `problem_complexity`: Difficulty weight of the problem
- `bounty`: Token reward for solving (in smallest unit)
- `time_asymmetry`: Ratio of solve time to verify time
- `space_asymmetry`: Ratio of solve memory to verify memory
- `solve_energy_joules`: Energy consumed during solution
- `verify_energy_joules`: Energy consumed during verification
- `total_energy_joules`: Total energy consumed
- `energy_asymmetry`: Ratio of solve energy to verify energy
- `work_score`: Computed work score based on asymmetries
- `block_height`: Blockchain height where solution was accepted
- `timestamp`: Unix timestamp of solution
- `status`: Problem status (Mined, Validated, etc.)
- `energy_measurement_method`: Method used for energy measurement
- `submission_mode`: How problem was submitted (mining, public, private)
""",
        "tags": ["tsp", "traveling-salesman", "np-hard", "blockchain", "proof-of-work"],
    },
    "SubsetSum": {
        "full_name": "Subset Sum Problem",
        "description": "Subset Sum problem instances and solutions from COINjecture Network B blockchain mining",
        "schema_description": """
### Schema

Each record contains:
- `problem_id`: Unique identifier for the problem instance
- `problem_type`: "SubsetSum"
- `problem_data`: Subset Sum problem specification
  - `numbers`: List of integers
  - `target`: Target sum to achieve
- `solution_data`: Subset Sum solution (indices)
  - `indices`: List of indices from numbers array that sum to target
- `problem_complexity`: Difficulty weight of the problem
- `bounty`: Token reward for solving (in smallest unit)
- `time_asymmetry`: Ratio of solve time to verify time
- `space_asymmetry`: Ratio of solve memory to verify memory
- `solve_energy_joules`: Energy consumed during solution
- `verify_energy_joules`: Energy consumed during verification
- `total_energy_joules`: Total energy consumed
- `energy_asymmetry`: Ratio of solve energy to verify energy
- `work_score`: Computed work score based on asymmetries
- `block_height`: Blockchain height where solution was accepted
- `timestamp`: Unix timestamp of solution
- `status`: Problem status (Mined, Validated, etc.)
- `energy_measurement_method`: Method used for energy measurement
- `submission_mode`: How problem was submitted (mining, public, private)
""",
        "tags": ["subset-sum", "np-complete", "blockchain", "proof-of-work"],
    },
    "Custom": {
        "full_name": "Custom Problem Type",
        "description": "Custom problem instances and solutions from COINjecture Network B blockchain",
        "schema_description": """
### Schema

Each record contains:
- `problem_id`: Unique identifier for the problem instance
- `problem_type`: "Custom"
- `problem_data`: Custom problem specification (base64-encoded)
  - `problem_id`: Problem identifier
  - `data`: Base64-encoded problem data
- `solution_data`: Custom solution (base64-encoded)
  - `data`: Base64-encoded solution data
- `problem_complexity`: Difficulty weight of the problem
- `bounty`: Token reward for solving (in smallest unit)
- `time_asymmetry`: Ratio of solve time to verify time
- `space_asymmetry`: Ratio of solve memory to verify memory
- `solve_energy_joules`: Energy consumed during solution
- `verify_energy_joules`: Energy consumed during verification
- `total_energy_joules`: Total energy consumed
- `energy_asymmetry`: Ratio of solve energy to verify energy
- `work_score`: Computed work score based on asymmetries
- `block_height`: Blockchain height where solution was accepted
- `timestamp`: Unix timestamp of solution
- `status`: Problem status (Mined, Validated, etc.)
- `energy_measurement_method`: Method used for energy measurement
- `submission_mode`: How problem was submitted (mining, public, private)
""",
        "tags": ["custom", "blockchain", "proof-of-work"],
    },
}


def create_readme(problem_type: str, config: dict) -> str:
    """Generate README content for a problem-type dataset"""
    return f"""---
license: mit
task_categories:
- other
language:
- en
tags:
{chr(10).join(f'- {tag}' for tag in config['tags'])}
size_categories:
- n<1K
---

# {DATASET_PREFIX}/{problem_type}_Solutions

## Dataset Description

**{config['full_name']}** solutions from the COINjecture Network B blockchain.

{config['description']}

This dataset contains real-world computational problem instances and their solutions that were generated and validated through blockchain consensus using Proof-of-Useful-Work (PoUW).

## Dataset Structure

{config['schema_description']}

### Data Fields

All records include comprehensive metrics:
- **Asymmetry Metrics**: Time, space, and energy asymmetries between solving and verification
- **Energy Measurements**: Detailed energy consumption during computation
- **Work Scores**: Blockchain consensus work scores based on computational asymmetries
- **Blockchain Metadata**: Block height, timestamps, miner addresses

### Data Splits

This is a continuously growing dataset with new solutions added as they are mined on the blockchain.

## Dataset Creation

### Source Data

Solutions are generated through the COINjecture Network B blockchain's mining process, where miners solve NP-hard problems to mine blocks.

### Energy Measurement

Energy measurements use platform-specific methods:
- **Linux**: RAPL (Running Average Power Limit) interface
- **macOS**: powermetrics
- **Fallback**: CPU TDP-based estimation

## Considerations for Using the Data

### Bias and Limitations

- Problem difficulty varies based on blockchain difficulty adjustment
- Energy measurements may use estimation on platforms without hardware monitoring
- Solutions represent successfully mined blocks only (failed attempts not recorded)

## Additional Information

### Dataset Curators

COINjecture Network B - Autonomous blockchain dataset generation

### Licensing

MIT License

### Citation

```bibtex
@misc{{coinjecture_netb_{problem_type.lower()},
  author = {{COINjecture Network B}},
  title = {{{config['full_name']} Solutions from Proof-of-Useful-Work Blockchain}},
  year = {{2025}},
  publisher = {{Hugging Face}},
  url = {{https://huggingface.co/datasets/{DATASET_PREFIX}/{problem_type}_Solutions}}
}}
```

### Contact

For issues or questions, please open an issue on the [COINjecture GitHub repository](https://github.com/Quigles1337/COINjecture1337-REFACTOR).

---

**Generated automatically by COINjecture Network B blockchain nodes**
"""


def create_dataset(problem_type: str, config: dict, api: HfApi):
    """Create a dataset repository with proper metadata"""
    dataset_name = f"{DATASET_PREFIX}/{problem_type}_Solutions"

    print(f"\n[*] Creating dataset: {dataset_name}")

    try:
        # Create the dataset repository
        api.create_repo(
            repo_id=dataset_name,
            repo_type="dataset",
            private=False,
            token=HF_TOKEN,
        )
        print(f"    [+] Repository created")

        # Generate and upload README
        readme_content = create_readme(problem_type, config)
        api.upload_file(
            path_or_fileobj=readme_content.encode('utf-8'),
            path_in_repo="README.md",
            repo_id=dataset_name,
            repo_type="dataset",
            token=HF_TOKEN,
        )
        print(f"    [+] README.md uploaded")

        # Create data directory with .gitkeep
        api.upload_file(
            path_or_fileobj=b"",
            path_in_repo="data/.gitkeep",
            repo_id=dataset_name,
            repo_type="dataset",
            token=HF_TOKEN,
        )
        print(f"    [+] data/ directory created")

        print(f"    [SUCCESS] Dataset {dataset_name} created successfully!")
        print(f"    [URL] https://huggingface.co/datasets/{dataset_name}")

        return True

    except Exception as e:
        if "Repository already exists" in str(e):
            print(f"    [WARN] Dataset already exists, skipping")
            return True
        else:
            print(f"    [ERROR] Failed to create dataset: {e}")
            return False


def main():
    print("=" * 70)
    print("COINjecture Network B - Institutional-Grade Dataset Initialization")
    print("=" * 70)
    print(f"\nDataset Prefix: {DATASET_PREFIX}")
    print(f"Problem Types: {len(PROBLEM_TYPES)}")
    print()

    api = HfApi()

    # Verify authentication
    try:
        user = api.whoami(token=HF_TOKEN)
        print(f"[+] Authenticated as: {user['name']}")
    except Exception as e:
        print(f"[-] Authentication failed: {e}")
        sys.exit(1)

    # Create each problem-type dataset
    success_count = 0
    for problem_type, config in PROBLEM_TYPES.items():
        if create_dataset(problem_type, config, api):
            success_count += 1

    print("\n" + "=" * 70)
    print(f"[SUCCESS] Created/verified {success_count}/{len(PROBLEM_TYPES)} datasets")
    print("=" * 70)

    print("\n[DATASETS] Dataset URLs:")
    for problem_type in PROBLEM_TYPES.keys():
        print(f"   - https://huggingface.co/datasets/{DATASET_PREFIX}/{problem_type}_Solutions")

    print("\n[READY] Ready for blockchain uploads!")
    print("   Nodes can now upload to these datasets automatically.")


if __name__ == "__main__":
    main()
