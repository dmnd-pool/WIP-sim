#!/usr/bin/env python3
"""Collect a pinned public archive. Folder names do not prove compatibility."""
import concurrent.futures
import hashlib
import json
from pathlib import Path
import urllib.parse
from fetch_artifact import ROOT, fetch

REPO = "asicminingsupplies/ASIC-Firmware-Repository"

def bitcoin_candidate(path):
    # This is a collection filter. Archive paths remain unverified technical evidence.
    p = path.casefold()
    if p.startswith(("bitmain/antminer/sha-256/", "canaan/", "whatsminer/", "ipollo/b series/")):
        return True
    if p.startswith("innosilicon/"):
        if p.startswith("innosilicon/tools/a4"):
            return False
        return p.startswith("innosilicon/t") or (p.startswith("innosilicon/manuals/") and ("btc" in p or "errorlist" in p)) or p.endswith("readme.md")
    return "/" not in path

def main():
    tree_manifest = json.loads((ROOT / "artifacts/manifests/firmware-archive-tree.json").read_text())
    tree = json.loads((ROOT / tree_manifest["local_path"]).read_text())
    if tree.get("truncated"):
        raise SystemExit("The GitHub tree is truncated. Do not call this a complete archive.")
    commit = tree["sha"]
    files = [entry for entry in tree["tree"] if entry["type"] == "blob"]
    out = ROOT / "catalog/discovery"
    out.mkdir(parents=True, exist_ok=True)
    inventory = []
    for entry in files:
        name = entry["path"]
        short = hashlib.sha256(name.encode()).hexdigest()[:16]
        suffix = name.lower()
        kind = "document" if suffix.endswith((".pdf", ".doc", ".docx", ".txt", ".csv", ".xls", ".xlsx")) else "firmware"
        if suffix.endswith((".md", ".gitattributes", "license")) or Path(name).name.startswith("."):
            kind = "metadata"
        if suffix.endswith((".exe", ".msi")) or "/tools/" in suffix:
            kind = "tool"
        inventory.append({"id": "mirror-" + short, "archive_path": name,
                          "vendor_folder": name.split("/")[0], "kind": kind,
                          "expected_size": entry.get("size"), "git_blob_sha1": entry["sha"],
                          "git_commit": commit, "repository": REPO,
                          "url": f"https://raw.githubusercontent.com/{REPO}/{commit}/" + urllib.parse.quote(name, safe="/"),
                          "evidence_status": "archive-label-only",
                          "scope": "bitcoin-asic" if bitcoin_candidate(name) else "supplementary-out-of-scope",
                          "notes": ["The archive path is not a verified model, algorithm, release date, or compatibility statement.",
                                    "The archive licence does not establish the licence of each vendor binary."]})
    (out / "firmware-archive-files.jsonl").write_text("".join(json.dumps(x, ensure_ascii=False) + "\n" for x in inventory))
    def collect(entry):
        result = fetch(entry["url"], entry["id"], entry["kind"], entry["archive_path"],
                       "und" if entry["kind"] in {"firmware", "tool"} else "en",
                       source_id="firmware-archive", max_bytes=1024**3)
        if result["status"] == "downloaded":
            result["kind"] = entry["kind"]
            path = ROOT / result["local_path"]
            digest = hashlib.sha1(f"blob {path.stat().st_size}\0".encode())
            with path.open("rb") as handle:
                while block := handle.read(1024*1024):
                    digest.update(block)
            result["git_blob_sha1"] = digest.hexdigest()
            result["matches_tree_blob"] = digest.hexdigest() == entry["git_blob_sha1"]
            result["matches_tree_size"] = result["size_bytes"] == entry["expected_size"]
            result["git_commit"] = commit
            result["archive_path"] = entry["archive_path"]
            (ROOT / "artifacts/manifests" / (entry["id"] + ".json")).write_text(json.dumps(result, indent=2) + "\n")
        return {key: result.get(key) for key in ["id", "status", "size_bytes", "matches_tree_blob", "error"]}
    selected = [entry for entry in inventory if bitcoin_candidate(entry["archive_path"])]
    scope_dir = ROOT / "catalog/parts/foundation"
    scope_dir.mkdir(parents=True, exist_ok=True)
    (scope_dir / "artifact-scope.jsonl").write_text("".join(json.dumps({"artifact_id": e["id"], "scope": e["scope"], "reason": "Bitcoin-only collection filter. The archive path is still unverified."}) + "\n" for e in inventory))
    with concurrent.futures.ThreadPoolExecutor(max_workers=10) as pool:
        for result in pool.map(collect, selected):
            print(json.dumps(result), flush=True)

if __name__ == "__main__":
    main()
