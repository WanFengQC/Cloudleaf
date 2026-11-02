"""只读重放合法源读音隔离，记录字节事实，不改变生产音节表。"""

from __future__ import annotations

from collections import Counter, defaultdict
from pathlib import Path
import re
import unicodedata
from zipfile import ZipFile

from .pipeline import Audit, SYLLABLES, canonical, digest, import_unihan, load_lock


def audit_readings(lock_path: Path, source: Path) -> dict:
    lock = load_lock(lock_path)
    if lock["format"] != "unihan-zip" or digest(source.read_bytes()) != lock["sha256"]:
        raise ValueError("需要已锁定的Unihan原件")
    audit = Audit()
    accepted = import_unihan(source, lock, audit)
    isolated = [row for row in audit.rejections if row["reason"] == "unsupported_reading"]
    codes = {row["raw"].split("\t")[0] for row in isolated}
    properties = defaultdict(list)
    with ZipFile(source) as archive:
        for member in sorted(archive.namelist()):
            for line, raw in enumerate(archive.read(member).decode("utf-8").splitlines(), 1):
                values = raw.split("\t")
                if len(values) == 3 and values[0] in codes and values[1] in {
                    "kMandarin", "kHanyuPinyin", "kXHC1983", "kTGHZ2013", "kDefinition"}:
                    properties[values[0]].append({"property": values[1], "value": values[2], "member": member, "line": line})
    coverage = defaultdict(list)
    for row in accepted:
        coverage[row["text"]].append(row["raw_reading"])
    rows = []
    for rejection in isolated:
        code, _, raw = rejection["raw"].split("\t")
        text = chr(int(code[2:], 16))
        nfd = unicodedata.normalize("NFD", raw)
        stripped = unicodedata.normalize("NFC", "".join(char for char in nfd if char not in "\u0300\u0301\u0304\u030c"))
        group = "syllabic-nasal-or-interjection" if stripped in {"n", "m", "ng", "hm"} else "inventory-gap"
        rows.append({"codepoint": code, "text": text, "raw_reading": raw,
                     "nfd_codepoints": [f"U+{ord(char):04X}" for char in nfd],
                     "uax38_rev39_syntax_matches": re.fullmatch(r"[a-z\u0300-\u0302\u0304\u0308\u030c]+", nfd) is not None,
                     "tone_removed": stripped, "in_current_inventory": stripped in SYLLABLES,
                     "classification": group, "source_location": {"member": rejection["member"], "line": rejection["line"]},
                     "source_properties": properties[code], "accepted_other_readings": sorted(coverage[text]),
                     "impact": "character-has-another-runtime-reading" if coverage[text] else "character-absent-from-runtime",
                     "decision": "isolate-with-evidence; no approximate substitution; keyboard spelling and linguistic adoption pending review"})
    return {"schema_version": 1, "source": lock["source_id"], "source_sha256": lock["sha256"],
            "specification": "https://www.unicode.org/reports/tr38/tr38-39.html#kMandarin",
            "license_id": lock["license_id"], "license_sha256": lock["license_sha256"],
            "syllables_sha256": digest(canonical(sorted(SYLLABLES))),
            "normalization_unicode_version": unicodedata.unidata_version,
            "reading_count": len(rows), "classifications": dict(Counter(row["classification"] for row in rows)),
            "impact_counts": dict(Counter(row["impact"] for row in rows)), "rows": rows,
            "interpretation": "UAX38 syntax and original properties are source facts; no assertion that an absent syllable is invalid Chinese, nor an adoption decision or measured usage frequency."}
