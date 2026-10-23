#!/usr/bin/env python3
"""Extract model names from saved public indexes. These are discovery records."""
import json
import re
from pathlib import Path
from fetch_artifact import ROOT

def objects_from_next_html(text):
    chunks = []
    for match in re.findall(r"self\.__next_f\.push\((.*?)\)</script>", text):
        try:
            value = json.loads(match)
            if len(value) > 1 and isinstance(value[1], str):
                chunks.append(value[1])
        except (ValueError, TypeError):
            pass
    for line in "".join(chunks).splitlines():
        try:
            yield json.loads(line.split(":", 1)[1])
        except (ValueError, IndexError):
            pass

def walk(value):
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from walk(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk(child)

def main():
    records = {}
    for name in ["minerstat", "asicminervalue"]:
        manifest = json.loads((ROOT / f"artifacts/manifests/{name}-discovery.json").read_text())
        content = (ROOT / manifest["local_path"]).read_text()
        for value in objects_from_next_html(content):
            for row in walk(value):
                if name == "minerstat" and all(k in row for k in ["slug", "model", "manufacturer", "type"]):
                    record = {"id": "discovery-ms-" + row["slug"], "model": row["model"],
                              "manufacturer": row["manufacturer"], "device_type": row["type"],
                              "reported_release_year": row.get("releaseYear"),
                              "reported_algorithm": row.get("bestAlgo"),
                              "url": "https://minerstat.com/hardware/" + row["slug"]}
                elif name == "asicminervalue" and all(k in row for k in ["slug", "name", "manufacturer", "hashrates"]):
                    record = {"id": "discovery-amv-" + row["slug"], "model": row["name"],
                              "manufacturer": row["manufacturer"].get("name"), "device_type": "ASIC",
                              "reported_release_date": str(row.get("firstRelease", "")).removeprefix("$D"),
                              "reported_algorithms": [h.get("algorithm", {}).get("name") for h in row["hashrates"]],
                              "url": "https://www.asicminervalue.com/miners/" + row["manufacturer"]["slug"] + "/" + row["slug"]}
                else:
                    continue
                record.update(source_id=name + "-discovery", evidence_status="unverified-discovery",
                              notes=["This secondary index is a search lead. Verify the model, date, and algorithm with a primary source.",
                                     "This record does not establish firmware or hardware compatibility."])
                algorithms = record.get("reported_algorithms") or [record.get("reported_algorithm")]
                is_bitcoin = record["device_type"] == "ASIC" and any(str(a).lower().replace("-", "") in {"sha256", "sha256d"} for a in algorithms)
                record["scope"] = "bitcoin-asic" if is_bitcoin else "supplementary-out-of-scope"
                records[record["id"]] = record
    dest = ROOT / "catalog/discovery/model-candidates.jsonl"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text("".join(json.dumps(row, ensure_ascii=False) + "\n" for row in records.values()))
    print(json.dumps({"model_candidates": len(records)}))

if __name__ == "__main__":
    main()
