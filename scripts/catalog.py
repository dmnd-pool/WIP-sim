#!/usr/bin/env python3
"""Build, search, and check the local mining catalogue. Uses Python's standard library."""
import argparse
from collections import Counter
import csv
import datetime as dt
import hashlib
import html
import json
from pathlib import Path
import sqlite3
import sys

ROOT = Path(__file__).resolve().parents[1]
TABLES = ("entities", "relationships", "sources", "gaps")

def rows(path):
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]

def translation_artifact_ids(row):
    return list(dict.fromkeys(([row["artifact_id"]] if row.get("artifact_id") else []) + row.get("artifact_ids", [])))

def load_parts(root=ROOT):
    data = {table: [] for table in TABLES}
    for table in TABLES:
        for path in sorted((root / "catalog/parts").glob("*/" + table + ".jsonl")):
            for row in rows(path):
                row["catalog_part"] = path.parent.name
                data[table].append(row)
    data["artifacts"] = []
    for path in sorted((root / "artifacts/manifests").glob("*.json")):
        data["artifacts"].append(json.loads(path.read_text()))
    inspections = {r["artifact_id"]: r for r in rows(root / "reports/artifact-inspection.jsonl")}
    esp_layouts = {r["artifact_id"]: r for r in rows(root / "catalog/parts/open/esp-image-layouts.jsonl")}
    translations = {}
    for row in rows(root / "translations/register.jsonl"):
        for artifact_id in translation_artifact_ids(row):
            translations.setdefault(artifact_id, []).append(row)
    for artifact in data["artifacts"]:
        inspection = inspections.get(artifact["id"])
        if inspection and inspection.get("sha256") == artifact.get("sha256"):
            artifact["inspection"] = inspection
        layout = esp_layouts.get(artifact["id"])
        if layout and layout.get("artifact_sha256") == artifact.get("sha256"):
            artifact["esp_image_layout"] = layout
        companions = [r for r in translations.get(artifact["id"], [])
                      if not r.get("source_sha256") or r["source_sha256"] == artifact.get("sha256")]
        if companions:
            artifact["english_companions"] = companions
    data["discovery"] = []
    for path in sorted((root / "catalog/discovery").glob("*.jsonl")):
        for row in rows(path):
            row["discovery_file"] = str(path.relative_to(root))
            data["discovery"].append(row)
    return data

def split_scope(data, root=ROOT):
    """Keep Bitcoin ASICs and their required evidence in the main catalogue."""
    allowed = {"bitcoin-asic", "supporting-tool", "mixed-reference"}
    scopes = {}
    for path in sorted((root / "catalog/parts").glob("*/artifact-scope.jsonl")):
        for row in rows(path):
            scopes[row["artifact_id"]] = row
    main = {key: [] for key in data}
    main["entities"] = [row for row in data["entities"] if row.get("scope") in allowed]
    entity_ids = {row["id"] for row in main["entities"]}
    main["relationships"] = [row for row in data["relationships"] if row["from_id"] in entity_ids and row["to_id"] in entity_ids]
    source_ids = {sid for row in main["entities"] + main["relationships"] for sid in row.get("source_ids", [])}
    main["gaps"] = [row for row in data["gaps"] if row.get("scope") in allowed or
                    (row.get("scope") != "supplementary-out-of-scope" and any(sid in source_ids for sid in row.get("source_ids", [])))]
    source_ids |= {sid for row in main["gaps"] for sid in row.get("source_ids", [])}
    main["sources"] = [row for row in data["sources"] if row["id"] in source_ids or row.get("scope") in allowed]
    artifact_ids = {aid for row in main["entities"] + main["sources"] for aid in row.get("artifact_ids", [])}
    for row in data["artifacts"]:
        row.update({"scope": scopes.get(row["id"], {}).get("scope", row.get("scope", "unclassified"))})
        if row["scope"] in allowed or (row["id"] in artifact_ids and row["scope"] != "supplementary-out-of-scope"):
            main["artifacts"].append(row)
    # A broad source can describe many products. Keep only the relevant file links in this view.
    main_artifact_ids = {row["id"] for row in main["artifacts"]}
    main["sources"] = [{**row, "artifact_ids": [aid for aid in row.get("artifact_ids", []) if aid in main_artifact_ids]} for row in main["sources"]]
    main["discovery"] = [row for row in data["discovery"] if row.get("scope") in allowed]
    included = {key: {row["id"] for row in records} for key, records in main.items()}
    supplementary = {key: [row for row in data[key] if row["id"] not in included[key]] for key in data}
    return main, supplementary

def validate(data, root=ROOT, hashes=False):
    errors = []
    storage_roots = [root.resolve()]
    storage_config = root / "catalog/storage.json"
    if storage_config.exists():
        storage_roots += [Path(p).resolve() for p in json.loads(storage_config.read_text()).get("artifact_roots", [])]
    ids = {}
    for table, records in data.items():
        keys = [r.get("id") for r in records]
        if None in keys:
            errors.append(f"{table}: a record has no ID")
        duplicates = [k for k, v in Counter(keys).items() if v > 1]
        errors.extend(f"{table}: duplicate ID {key}" for key in duplicates)
        ids[table] = set(keys)
    required = {"entities": ["id", "kind", "name", "manufacturer", "source_ids"],
                "relationships": ["id", "from_id", "to_id", "relation", "status", "source_ids"],
                "sources": ["id", "url", "title", "source_type"],
                "gaps": ["id", "area", "description", "reason", "next_action"]}
    for table, fields in required.items():
        for row in data.get(table, []):
            for field in fields:
                if field not in row:
                    errors.append(f"{table}/{row.get('id')}: missing {field}")
            for key in row.get("source_ids", []):
                if key not in ids.get("sources", set()):
                    errors.append(f"{table}/{row['id']}: unknown source {key}")
            for key in row.get("artifact_ids", []):
                if key not in ids.get("artifacts", set()):
                    errors.append(f"{table}/{row['id']}: unknown artifact {key}")
            if table == "relationships":
                for end in ["from_id", "to_id"]:
                    if row.get(end) not in ids.get("entities", set()):
                        errors.append(f"relationship/{row['id']}: unknown {end} {row.get(end)}")
            if table == "entities":
                year = row.get("release_year")
                if year is not None and (not isinstance(year, int) or not 1990 <= year <= 2100):
                    errors.append(f"entity/{row['id']}: invalid release year {year}")
    for row in data.get("artifacts", []):
        if row.get("status") != "downloaded":
            continue
        rel = row.get("local_path")
        if not rel:
            errors.append(f"artifact/{row['id']}: downloaded file has no path")
            continue
        path = (root / rel).resolve()
        if Path(rel).is_absolute() or ".." in Path(rel).parts or not any(path.is_relative_to(r) for r in storage_roots):
            errors.append(f"artifact/{row['id']}: path leaves the catalogue")
            continue
        if not path.is_file():
            errors.append(f"artifact/{row['id']}: file is missing")
        elif path.stat().st_size != row.get("size_bytes"):
            errors.append(f"artifact/{row['id']}: file size changed")
        elif hashes:
            digest = hashlib.sha256()
            with path.open("rb") as handle:
                while block := handle.read(1024*1024):
                    digest.update(block)
            if digest.hexdigest() != row.get("sha256"):
                errors.append(f"artifact/{row['id']}: SHA-256 mismatch")
        if row.get("matches_tree_blob") is False:
            errors.append(f"artifact/{row['id']}: Git archive blob mismatch")
        if row.get("matches_lfs_oid") is False or row.get("matches_lfs_size") is False:
            errors.append(f"artifact/{row['id']}: Git LFS object mismatch")
    return errors

def cell(value):
    if isinstance(value, (dict, list)):
        return json.dumps(value, ensure_ascii=False)
    return "" if value is None else value

def write_csv(path, records, fields):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, extrasaction="ignore")
        writer.writeheader()
        writer.writerows({key: cell(row.get(key)) for key in fields} for row in records)

def build(root=ROOT):
    all_data = load_parts(root)
    data, supplementary = split_scope(all_data, root)
    errors = validate(data, root)
    if errors:
        print("\n".join(errors[:100]), file=sys.stderr)
        raise SystemExit(f"Validation failed: {len(errors)} errors.")
    catalog = root / "catalog"
    supplement_dir = catalog / "supplementary"
    supplement_dir.mkdir(exist_ok=True)
    for table, records in supplementary.items():
        (supplement_dir / (table + ".jsonl")).write_text("".join(json.dumps(row, ensure_ascii=False) + "\n" for row in records))
    for table, records in data.items():
        if table != "discovery":
            (catalog / (table + ".jsonl")).write_text("".join(json.dumps(row, ensure_ascii=False) + "\n" for row in records))
    dbpath = catalog / "catalog.sqlite"
    tmp = dbpath.with_suffix(".sqlite.tmp")
    tmp.unlink(missing_ok=True)
    con = sqlite3.connect(tmp)
    con.executescript("""
      CREATE TABLE entities(id TEXT PRIMARY KEY, kind TEXT, name TEXT, manufacturer TEXT, release_year INTEGER, date_basis TEXT, json TEXT);
      CREATE TABLE relationships(id TEXT PRIMARY KEY, from_id TEXT, to_id TEXT, relation TEXT, status TEXT, json TEXT);
      CREATE TABLE sources(id TEXT PRIMARY KEY, title TEXT, url TEXT, source_type TEXT, json TEXT);
      CREATE TABLE artifacts(id TEXT PRIMARY KEY, kind TEXT, title TEXT, status TEXT, size_bytes INTEGER, local_path TEXT, sha256 TEXT, json TEXT);
      CREATE TABLE gaps(id TEXT PRIMARY KEY, area TEXT, description TEXT, priority TEXT, json TEXT);
      CREATE TABLE discovery(id TEXT PRIMARY KEY, json TEXT);
      CREATE INDEX rel_from ON relationships(from_id);
      CREATE INDEX rel_to ON relationships(to_id);
      CREATE INDEX entity_year ON entities(release_year);
      CREATE INDEX entity_maker ON entities(manufacturer);
      CREATE INDEX artifact_hash ON artifacts(sha256);
      CREATE VIRTUAL TABLE search_index USING fts5(id UNINDEXED, category UNINDEXED, text);
    """)
    for table, records in data.items():
        fields = [r[1] for r in con.execute(f"PRAGMA table_info({table})")]
        for row in records:
            raw = json.dumps(row, ensure_ascii=False)
            values = [raw if key == "json" else row.get(key) for key in fields]
            con.execute(f"INSERT INTO {table} VALUES ({','.join('?' for _ in fields)})", values)
            con.execute("INSERT INTO search_index VALUES (?,?,?)", (row["id"], table, raw))
    con.commit()
    con.close()
    tmp.replace(dbpath)
    csvdir = catalog / "csv"
    write_csv(csvdir / "entities.csv", data["entities"], ["id", "kind", "name", "manufacturer", "release_year", "date_basis", "algorithms", "purpose", "hardware", "software", "source_ids", "artifact_ids", "notes"])
    write_csv(csvdir / "relationships.csv", data["relationships"], ["id", "from_id", "to_id", "relation", "status", "firmware_version", "hardware_revision", "constraints", "source_ids", "notes"])
    write_csv(csvdir / "artifacts.csv", data["artifacts"], ["id", "kind", "title", "status", "url", "local_path", "size_bytes", "sha256", "language", "license", "retrieved_at", "error"])
    write_csv(csvdir / "sources.csv", data["sources"], ["id", "title", "publisher", "url", "source_type", "language", "artifact_ids", "notes"])
    write_csv(csvdir / "gaps.csv", data["gaps"], ["id", "area", "description", "reason", "priority", "next_action", "source_ids"])
    write_csv(csvdir / "discovery.csv", data["discovery"], sorted({key for row in data["discovery"] for key in row}))
    byid = {e["id"]: e for e in data["entities"]}
    matrix = []
    for row in data["relationships"]:
        if row["relation"] in {"supports", "builds-for", "incompatible-with"}:
            matrix.append({**row, "from_name": byid[row["from_id"]]["name"], "to_name": byid[row["to_id"]]["name"]})
    write_csv(csvdir / "compatibility.csv", matrix, ["from_id", "from_name", "to_id", "to_name", "relation", "status", "firmware_version", "hardware_revision", "constraints", "source_ids"])
    stats = {"generated_at": dt.datetime.now(dt.timezone.utc).isoformat(), "scope": "Bitcoin ASIC miners only",
             "counts": {table: len(items) for table, items in data.items()},
             "entity_kinds": dict(Counter(e["kind"] for e in data["entities"])),
             "manufacturers": dict(Counter(e["manufacturer"] for e in data["entities"] if e["kind"] == "device")),
             "download_status": dict(Counter(a["status"] for a in data["artifacts"])),
             "git_lfs_pointer_files": sum(a.get("content_role") == "git-lfs-pointer" for a in data["artifacts"]),
             "firmware_payload_files": sum(a.get("kind") == "firmware" and a.get("status") == "downloaded" and a.get("content_role") != "git-lfs-pointer" and a.get("payload_available") is not False for a in data["artifacts"]),
             "unique_downloaded_sha256": len({a["sha256"] for a in data["artifacts"] if a["status"] == "downloaded"}),
             "downloaded_bytes": sum(a.get("size_bytes") or 0 for a in data["artifacts"] if a["status"] == "downloaded"),
             "unique_downloaded_bytes": sum({a["sha256"]: a["size_bytes"] for a in data["artifacts"] if a["status"] == "downloaded"}.values()),
             "known_device_release_years": sum(e.get("release_year") is not None for e in data["entities"] if e["kind"] == "device"),
             "locally_tested_compatibility_edges": sum(r.get("status") == "tested" for r in data["relationships"]),
             "supplementary_counts": {key: len(items) for key, items in supplementary.items()}}
    (root / "reports/statistics.json").write_text(json.dumps(stats, indent=2) + "\n")
    template = (root / "scripts/browser-template.html").read_text()
    # Escape HTML delimiters so source text cannot close a script element.
    browser_data = json.dumps(data, ensure_ascii=False).replace("<", "\\u003c").replace(">", "\\u003e").replace("&", "\\u0026")
    (catalog / "index.html").write_text(template.replace("__CATALOG_DATA__", browser_data))
    indexes = root / "docs/indexes"
    indexes.mkdir(exist_ok=True)
    for kind in sorted(stats["entity_kinds"]):
        lines = [f"# {kind.title()} index", "", "Unknown dates are shown as `unknown`.", "", "| ID | Name | Maker | Year |", "| --- | --- | --- | --- |"]
        for e in sorted((e for e in data["entities"] if e["kind"] == kind), key=lambda e: (e["manufacturer"], e["name"])):
            lines.append("| " + " | ".join(str(x).replace("|", "/") for x in [e["id"], e["name"], e["manufacturer"], e.get("release_year") or "unknown"]) + " |")
        (indexes / (kind + ".md")).write_text("\n".join(lines) + "\n")
    print(json.dumps(stats, indent=2))

def connection():
    path = ROOT / "catalog/catalog.sqlite"
    if not path.exists():
        raise SystemExit("Build the index first: python3 scripts/catalog.py build")
    return sqlite3.connect(f"file:{path}?mode=ro", uri=True)

def output(records, as_json=False):
    if as_json:
        print(json.dumps(records, indent=2, ensure_ascii=False))
    else:
        for row in records:
            print("\t".join(str(row.get(k) or "") for k in ["id", "kind", "name", "manufacturer", "release_year", "status", "local_path"]))

def assess_pair(relationships, firmware_id, hardware_id):
    """Report direct claims only. Do not infer compatibility through a family or board."""
    edges = [r for r in relationships if r.get("from_id") == firmware_id and r.get("to_id") == hardware_id
             and r.get("relation") in {"supports", "builds-for", "incompatible-with"}]
    negative = [r for r in edges if r["relation"] == "incompatible-with" or r.get("status") == "unsupported"]
    labels = [r for r in edges if r.get("status") == "archive-label-only"]
    positive = [r for r in edges if r not in negative and r not in labels and r.get("status") != "unknown"]
    verdict = "conflicting-claims" if negative and positive else "incompatible-claim" if negative else "support-claim" if positive else "target-label-only" if labels else "unknown"
    return {"firmware_id": firmware_id, "hardware_id": hardware_id, "result": verdict, "direct_claims": edges,
            "note": "This result reports evidence. It does not authorize flashing or prove that an emulator can run this pair. Read every constraint. Missing evidence means unknown."}

def main():
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("build")
    v = sub.add_parser("verify"); v.add_argument("--hashes", action="store_true")
    sub.add_parser("stats")
    for name in ["search", "artifacts", "gaps", "discovery"]:
        s = sub.add_parser(name); s.add_argument("query", nargs="?", default="")
        s.add_argument("--kind"); s.add_argument("--manufacturer"); s.add_argument("--year", type=int)
        s.add_argument("--status"); s.add_argument("--limit", type=int, default=50); s.add_argument("--json", action="store_true")
    s = sub.add_parser("show"); s.add_argument("id")
    s = sub.add_parser("links"); s.add_argument("id"); s.add_argument("--relation")
    s = sub.add_parser("check-pair"); s.add_argument("firmware_id"); s.add_argument("hardware_id")
    s = sub.add_parser("members"); s.add_argument("query"); s.add_argument("--artifact", default=""); s.add_argument("--limit", type=int, default=50)
    s = sub.add_parser("documents"); s.add_argument("query"); s.add_argument("--artifact", default=""); s.add_argument("--limit", type=int, default=20)
    args = p.parse_args()
    if args.cmd == "build":
        build(); return
    if args.cmd == "verify":
        all_data = load_parts(); data, supplementary = split_scope(all_data)
        errors = validate(data, hashes=args.hashes)
        # Check retained supplementary file identity as well, without presenting those records as Bitcoin scope.
        errors += validate({"artifacts": supplementary["artifacts"]}, hashes=args.hashes)
        report = {"checked_at": dt.datetime.now(dt.timezone.utc).isoformat(), "sha256_checked": args.hashes,
                  "errors": errors, "artifact_count": len(data["artifacts"]), "result": "pass" if not errors else "fail"}
        (ROOT / "reports/validation.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2)); raise SystemExit(bool(errors))
    if args.cmd == "stats":
        print((ROOT / "reports/statistics.json").read_text()); return
    if args.cmd == "documents":
        path = ROOT / "catalog/documents.sqlite"
        if not path.exists():
            raise SystemExit("Run scripts/index_documents.py first.")
        db = sqlite3.connect(f"file:{path}?mode=ro", uri=True); db.row_factory = sqlite3.Row
        try:
            found = db.execute("SELECT d.*,snippet(document_search,1,'[',']',' … ',40) AS excerpt FROM document_search JOIN documents d ON d.id=document_search.rowid WHERE document_search MATCH ? AND instr(d.artifact_id,?)>0 ORDER BY rank LIMIT ?", (args.query,args.artifact,max(1,args.limit))).fetchall()
        except sqlite3.OperationalError as exc:
            raise SystemExit("Invalid full-text query: " + str(exc))
        print(json.dumps([dict(row) for row in found], indent=2, ensure_ascii=False)); db.close(); return
    if args.cmd == "members":
        path = ROOT / "catalog/archive-members.sqlite"
        if not path.exists():
            raise SystemExit("Run scripts/index_archive_members.py first.")
        db = sqlite3.connect(f"file:{path}?mode=ro", uri=True); db.row_factory = sqlite3.Row
        rows = db.execute("SELECT m.artifact_id,m.path,m.size_bytes,m.member_type,a.sha256 AS parent_sha256,a.local_path AS parent_path,a.truncated AS member_list_truncated FROM members m JOIN archives a ON a.artifact_id=m.artifact_id WHERE instr(lower(m.path),lower(?))>0 AND instr(m.artifact_id,?)>0 LIMIT ?", (args.query,args.artifact,max(1,args.limit))).fetchall()
        print(json.dumps([dict(row) for row in rows], indent=2)); db.close(); return
    con = connection()
    if args.cmd == "check-pair":
        for ident in [args.firmware_id, args.hardware_id]:
            if not con.execute("SELECT 1 FROM entities WHERE id=?", (ident,)).fetchone():
                raise SystemExit("Entity ID not found: " + ident)
        edges = [json.loads(r[0]) for r in con.execute("SELECT json FROM relationships WHERE from_id=? AND to_id=?", (args.firmware_id, args.hardware_id))]
        print(json.dumps(assess_pair(edges, args.firmware_id, args.hardware_id), indent=2)); return
    if args.cmd == "show":
        found = []
        for table in [*TABLES, "artifacts", "discovery"]:
            found.extend(json.loads(r[0]) for r in con.execute(f"SELECT json FROM {table} WHERE id=?", (args.id,)))
        if not found:
            raise SystemExit("ID not found.")
        output(found, True); return
    if args.cmd == "links":
        query = "SELECT json FROM relationships WHERE (from_id=? OR to_id=?)"
        params = [args.id, args.id]
        if args.relation:
            query += " AND relation=?"; params.append(args.relation)
        output([json.loads(r[0]) for r in con.execute(query, params)], True); return
    table = "entities" if args.cmd == "search" else args.cmd
    records = [json.loads(r[0]) for r in con.execute(f"SELECT json FROM {table}")]
    query = args.query.casefold()
    records = [r for r in records if query in json.dumps(r, ensure_ascii=False).casefold()]
    for field in ["kind", "manufacturer", "release_year", "status"]:
        val = getattr(args, "year" if field == "release_year" else field, None)
        if val is not None:
            records = [r for r in records if str(r.get(field, "")).casefold() == str(val).casefold()]
    if table != "entities":
        for r in records:
            r.setdefault("name", r.get("title") or r.get("description") or r.get("archive_path") or r.get("model"))
    output(records[:args.limit], args.json)

if __name__ == "__main__":
    main()
