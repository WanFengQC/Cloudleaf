"""固定完整CC-CEDICT v1 ZIP；不复用镜像解析器，不推断词性或频率。"""

from __future__ import annotations

from pathlib import Path
import re
from zipfile import ZipFile

from .pipeline import Audit, MAX_EXPANDED, canonical, digest, make_record, normalize_reading


def source_key(traditional: str, simplified: str, raw_reading: str) -> str:
    """读音大小写属于源身份；英文义项修订不改变此来源键。"""
    return "cedict-v1:" + digest(canonical([traditional, simplified, raw_reading]))[:32]


def parse_reading(raw: str) -> list[str]:
    tokens = raw.split(" ")
    if any(not re.fullmatch(r"[A-Za-züÜ:]+[1-5]", token) for token in tokens):
        raise ValueError("unsupported_cedict_reading")
    return [normalize_reading(token) for token in tokens]


def read_archive(source: Path, lock: dict) -> tuple[str, dict]:
    with ZipFile(source) as archive:
        infos = archive.infolist()
        if len(infos) != 1:
            raise ValueError("CC-CEDICT归档必须只有一个已锁定成员")
        info = infos[0]
        if (info.filename != "cedict_ts.u8" or info.filename != lock.get("source_member")
                or info.is_dir() or info.flag_bits & 1 or info.file_size > MAX_EXPANDED
                or (info.external_attr >> 16) & 0o170000 == 0o120000):
            raise ValueError("CC-CEDICT成员名称、类型或体积错误")
        # ZipFile.read会校验完整成员CRC；不在磁盘展开归档路径。
        data = archive.read(info)
    if len(data) != lock.get("expanded_bytes") or digest(data) != lock.get("expanded_sha256"):
        raise ValueError("CC-CEDICT明文锁不匹配")
    text = data.decode("utf-8")
    if text.startswith("\ufeff") or "\x00" in text:
        raise ValueError("CC-CEDICT编码标记或控制字符错误")
    lines = text.splitlines()
    if any(len(line.encode("utf-8")) > 65536 for line in lines):
        raise ValueError("CC-CEDICT源行超限")
    header = {}
    for line in lines:
        if line.startswith("#! "):
            key, sep, value = line[3:].partition("=")
            if not sep or key in header:
                raise ValueError("CC-CEDICT头部格式或重复属性错误")
            header[key] = value
    if (header != lock.get("source_header") or header.get("version") != "1"
            or header.get("charset") != "UTF-8" or header.get("format") != "ts"
            or header.get("license") != "https://creativecommons.org/licenses/by-sa/4.0/"
            or header.get("publisher") != "MDBG"):
        raise ValueError("CC-CEDICT头部版本或授权不匹配")
    count = sum(bool(line) and not line.startswith("#") for line in lines)
    if str(count) != header.get("entries"):
        raise ValueError("CC-CEDICT完整源条数与头部不一致")
    return text, header


def import_cedict_archive(source: Path, lock: dict, audit: Audit) -> list[dict]:
    text, _header = read_archive(source, lock)
    pending: dict[str, list[dict]] = {}
    conflicting = set()
    for number, line in enumerate(text.splitlines(), 1):
        audit.counts["raw_lines"] += 1
        if not line or line.startswith("#"):
            continue
        audit.counts["source_entry_lines"] += 1
        match = re.fullmatch(r"([^ ]+) ([^ ]+) \[([^\[\]]+)\] /(.+)/", line)
        if not match:
            audit.reject("cedict_syntax", lock["source_member"], number, line)
            continue
        traditional, simplified, raw_reading, meanings = match.groups()
        key = source_key(traditional, simplified, raw_reading)
        try:
            pinyin = parse_reading(raw_reading)
            row = make_record(lock, key, simplified, pinyin, raw_reading, meanings.split("/"),
                              [{"member": lock["source_member"], "line": number, "raw": line}],
                              traditional=traditional, simplified=simplified)
        except ValueError as error:
            reason = str(error)
            if reason == "invalid_text_or_syllable_count":
                reason = "cedict_runtime_budget"
            audit.reject(reason, lock["source_member"], number, line)
            continue
        old = pending.setdefault(key, [])
        if old and old[0]["raw_senses"] != row["raw_senses"]:
            conflicting.add(key)
        old.append(row)
    records = []
    for key, rows in pending.items():
        if key in conflicting:
            for row in rows:
                item = row["evidence"][0]
                audit.reject("cedict_source_conflict", item["member"], item["line"], item["raw"])
            continue
        records.extend(rows)
    audit.counts["conflicting_lexemes"] = len(conflicting)
    return records


def derived_quality(records: list[dict]) -> dict[str, int]:
    """只由最终事实层推导的计数，在build与verify分别重算。"""
    return {"accepted_single_character_records": sum(len(row["text"]) == 1 for row in records),
            "accepted_multicharacter_records": sum(len(row["text"]) > 1 for row in records),
            "accepted_traditional_simplified_differences": sum(row["traditional"] != row["simplified"] for row in records),
            "accepted_multiple_senses_records": sum(len(row["raw_senses"]) > 1 for row in records),
            "accepted_uppercase_reading_records": sum(any(char.isupper() for char in row["raw_reading"]) for row in records),
            "accepted_non_han_headword_records": sum(any(not (0x3400 <= ord(char) <= 0x9FFF or 0x20000 <= ord(char) <= 0x3347F)
                                                         for char in row["text"]) for row in records)}
