"""从实装环境只读采集开发工具的许可证与元数据，不导入运行时或联网。"""
import base64
import hashlib
import importlib.metadata as metadata
import json
from pathlib import Path
import re
import sys

ROOT = Path(r"C:\Users\EDY\.codex\worktrees\translation-round11\Cloudleaf")
OUTPUT = Path(r"C:\Users\EDY\.codex\tmp\cloudleaf-r13-final-review\conversion-licenses")
SOURCE = Path(r"C:\Users\EDY\.codex\tmp\cloudleaf-r13-model-review\runtime-audit\official-reference")
OUTPUT.mkdir(parents=True, exist_ok=False)
LICENSE_NAME = re.compile(r"^(license|licence|notice|copying|copyright|authors)([._-]|$)", re.I)

SUMMARIES = {
    "filelock": "MIT；以实装4.2.4为准，不沿用早期候选Unlicense结论。",
    "fsspec": "BSD-3-Clause；未启用http/s3等extras。",
    "jinja2": "BSD-3-Clause；实装Pallets许可原文。",
    "markupsafe": "BSD-3-Clause。",
    "mpmath": "BSD-3-Clause；metadata仅写BSD，原文包含三条件。",
    "networkx": "BSD-3-Clause。",
    "numpy": "源码表达式BSD-3-Clause AND 0BSD AND MIT AND Zlib AND CC0-1.0；本Windows二进制LICENSE另附OpenBLAS BSD-3-Clause、LAPACK BSD-3-Clause-Open-MPI、GCC runtime GPL-3.0-or-later WITH GCC-exception-3.1及完整例外原文，不能只概括BSD。",
    "packaging": "Apache-2.0 OR BSD-2-Clause；保留两种许可与选择说明。",
    "pip": "MIT主许可；本任务归档实际RECORD所列许可/作者，不把pip工具本身等同产品依赖，也不声称已独立复核每项内嵌代码许可。",
    "safetensors": "Apache-2.0；实装许可证与固定上游原文分别保留。",
    "sentencepiece": "实装wheel未携带独立LICENSE或License-File元数据；补充此前已取得的固定官方源码Apache-2.0许可证，明确它不是wheel内文件。",
    "setuptools": "MIT主许可；保留vendored包及config NOTICE，存在Apache、BSD、PSF等多种第三方许可，不能概括全部MIT。",
    "sympy": "BSD主许可；原文含附带软件许可与作者清单，另有latex MIT许可。",
    "torch": "BSD-3-Clause主许可；实装544783B聚合LICENSE列出大量第三方MIT/BSD/Apache及例外等许可，另保留24088B NOTICE；CPU wheel许可清单包含部分GPU项目名称，不据此声称装有CUDA运行时。",
    "typing-extensions": "PSF-2.0，并保留原文所含相关版本许可与历史说明。",
}

def sha(data):
    return hashlib.sha256(data).hexdigest()

def key(name):
    return name.lower().replace("_", "-")

def archive(path, relative):
    content = path.read_bytes()
    destination = OUTPUT / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(content)
    return {"file": relative.as_posix(), "bytes": len(content), "sha256": sha(content)}

reports = []
install_origins = {}
for filename in ["torch-install-report.json", "conversion-extra-install-report.json"]:
    path = ROOT / ".local/round13-review" / filename
    archive(path, Path("acquisition") / filename)
    for row in json.loads(path.read_text(encoding="utf-8"))["install"]:
        install_origins[key(row["metadata"]["name"])] = row["download_info"]

for distribution in sorted(metadata.distributions(), key=lambda value: key(value.metadata["Name"])):
    name = distribution.metadata["Name"]
    normalized = key(name)
    version = distribution.version
    files = list(distribution.files or [])
    dist_info = next(Path(value).parts[0] for value in files if str(value).endswith(".dist-info/METADATA"))
    base = Path(normalized + "-" + version)
    matches = [value for value in files if LICENSE_NAME.match(value.name) or ".dist-info/licenses/" in str(value).replace("\\", "/")]
    archived = []
    for value in matches:
        source = Path(distribution.locate_file(value))
        record = archive(source, base / str(value))
        record["source_relative_to_site_packages"] = str(value).replace("\\", "/")
        if value.hash is not None:
            actual = hashlib.new(value.hash.mode, source.read_bytes()).digest()
            actual = base64.urlsafe_b64encode(actual).rstrip(b"=").decode("ascii")
            record["record_hash_matches"] = actual == value.hash.value
            if not record["record_hash_matches"]:
                raise RuntimeError("RECORD hash mismatch: " + str(value))
        else:
            record["record_hash_matches"] = None
        archived.append(record)
    evidence = []
    for filename in ["METADATA", "WHEEL", "INSTALLER", "RECORD", "direct_url.json"]:
        path = Path(distribution.locate_file(Path(dist_info) / filename))
        if path.is_file():
            evidence.append(archive(path, base / dist_info / filename))
    report = {
        "name": name,
        "version": version,
        "metadata_license_expression": distribution.metadata.get("License-Expression"),
        "metadata_license": distribution.metadata.get("License"),
        "declared_license_files": distribution.metadata.get_all("License-File", []),
        "requires_dist": distribution.metadata.get_all("Requires-Dist", []),
        "license_summary": SUMMARIES[normalized],
        "installed_license_files": archived,
        "metadata_evidence": evidence,
        "acquisition": install_origins.get(normalized),
        "status": "installed_originals_archived" if matches else "no_license_file_in_installed_wheel",
    }
    if normalized == "sentencepiece":
        source_manifest = json.loads((SOURCE / "manifest.json").read_text(encoding="utf-8"))
        supplemental = next(value for value in source_manifest["files"] if value["file"] == "sentencepiece-LICENSE")
        original = SOURCE / supplemental["file"]
        assert sha(original.read_bytes()) == supplemental["sha256"]
        report["upstream_license_supplement"] = archive(original, base / "supplemental-upstream/LICENSE") | {"source_url": supplemental["url"], "scope": "cached official fixed source license, not a file shipped in the wheel"}
        wheel = next(value for value in source_manifest["files"] if value["file"].endswith("win_amd64.whl"))
        report["acquisition"] = {"url": wheel["url"], "sha256": wheel["sha256"], "bytes": wheel["bytes"], "scope": "cached wheel from earlier acquisition; no new download"}
    reports.append(report)

record = {
    "schema": 1,
    "purpose": "actual installed development conversion and tokenizer differential tools only; no product runtime redistribution approval",
    "python": sys.version,
    "executable": sys.executable,
    "network_used": False,
    "collector_sha256": sha(Path(__file__).read_bytes()),
    "distributions": reports,
    "distribution_count": len(reports),
    "license_original_count": sum(len(value["installed_license_files"]) for value in reports),
    "missing_wheel_license": [value["name"] for value in reports if not value["installed_license_files"]],
    "record_hash_mismatches": 0,
}
(OUTPUT / "manifest.json").write_text(json.dumps(record, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
print(json.dumps({value: record[value] for value in ["distribution_count", "license_original_count", "missing_wheel_license", "record_hash_mismatches"]}, ensure_ascii=False))
