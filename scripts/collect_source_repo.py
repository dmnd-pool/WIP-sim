#!/usr/bin/env python3
"""Archive a GitHub source snapshot pinned to a commit. Do not run its code."""
import argparse
import json
import re
import subprocess
from fetch_artifact import ROOT, fetch

def collect(repo, artifact_id, commit=None, source_id=None):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        raise ValueError("Expected owner/repository.")
    prior_path = ROOT / "artifacts/manifests" / (artifact_id + ".json")
    if prior_path.exists():
        prior = json.loads(prior_path.read_text())
        if prior.get("status") == "downloaded" and prior.get("git_commit"):
            if prior.get("repository") != "https://github.com/" + repo or (commit and commit != prior["git_commit"]):
                raise ValueError("This artifact ID already identifies another source snapshot. Use a new ID.")
            return prior
    if not commit:
        result = subprocess.run(["git", "ls-remote", "https://github.com/" + repo + ".git", "HEAD"],
                                capture_output=True, text=True, timeout=90, check=True)
        commit = result.stdout.split()[0]
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("Expected a 40-character commit ID.")
    result = fetch(f"https://codeload.github.com/{repo}/tar.gz/{commit}", artifact_id,
                   "source", repo + " source snapshot", "mixed", filename=repo.split("/")[1] + "-" + commit[:12] + ".tar.gz",
                   source_id=source_id or artifact_id, max_bytes=2*1024**3)
    result.update(repository="https://github.com/" + repo, git_commit=commit,
                  submodules_included=False,
                  dependency_status="Snapshot only. Submodules, toolchains, and external dependencies need separate collection.")
    (ROOT / "artifacts/manifests" / (artifact_id + ".json")).write_text(json.dumps(result, indent=2) + "\n")
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repo")
    parser.add_argument("--id", required=True)
    parser.add_argument("--commit")
    args = parser.parse_args()
    print(json.dumps(collect(args.repo, args.id, args.commit)))

if __name__ == "__main__":
    main()
