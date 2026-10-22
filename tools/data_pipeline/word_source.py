"""只从明确普通话拼音与词头构建可追溯的真实词条，不推测频率。"""

from __future__ import annotations

import gzip
import json
from pathlib import Path
import re
import unicodedata
from urllib.parse import quote

from .pipeline import SYLLABLES, canonical, clean_field, digest, make_record, normalize_reading

MAX_WORD_SOURCE = 512 * 1024 * 1024
MAX_WORD_EXPANDED = 4 * 1024 * 1024 * 1024
MAX_WORD_LINE = 4 * 1024 * 1024
RIGHTS_KEYS = {"license", "licence", "copyright", "attribution", "source", "source_url", "references", "reference"}


def normalize_word_reading(raw: str) -> list[str]:
    """用显式分隔、声调位置与拼音隔音规则求唯一投影，歧义时拒绝。"""
    if not isinstance(raw, str) or not clean_field(raw) or len(raw) > 256:
        raise ValueError("unsupported_word_reading")
    raw = unicodedata.normalize("NFC", raw.lower().replace("’", "'"))
    parts = re.split("[ '\\-]", raw)
    if not all(parts):
        raise ValueError("unsupported_word_reading")
    result = []
    for part in parts:
        if not re.fullmatch(r"[a-züāáǎàēéěèīíǐìōóǒòūúǔùǖǘǚǜ]+", part):
            raise ValueError("unsupported_word_reading")
        paths = [[] for _ in range(len(part) + 1)]
        paths[0] = [()]
        for end in range(1, len(part) + 1):
            # a/e/o开头的新音节在词内必须有隔音符号，不能把hao猜成ha+o。
            if end < len(part) and unicodedata.normalize("NFD", part[end])[0] in "aeo":
                continue
            for start in range(max(0, end - 6), end):
                token = part[start:end]
                if sum(char in "\u0300\u0301\u0304\u030c" for char in unicodedata.normalize("NFD", token)) > 1:
                    continue
                try:
                    value = normalize_reading(token)
                except ValueError:
                    continue
                for prefix in paths[start]:
                    path = prefix + (value,)
                    if path not in paths[end]:
                        paths[end].append(path)
                paths[end] = paths[end][:2]
        if len(paths[-1]) != 1:
            raise ValueError("ambiguous_or_unsupported_word_reading")
        result.extend(paths[-1][0])
    if not 1 <= len(result) <= 16 or sum(map(len, result)) > 64:
        raise ValueError("word_reading_budget")
    return result


def rights_markers(value: object, path: str = "") -> list[dict]:
    result = []
    if isinstance(value, dict):
        for key, item in value.items():
            location = path + "/" + key
            if key.lower() in RIGHTS_KEYS and item:
                result.append({"path": location, "value": item})
            if key not in {"examples", "sounds"}:
                result.extend(rights_markers(item, location))
    elif isinstance(value, list):
        for number, item in enumerate(value):
            result.extend(rights_markers(item, path + "/" + str(number)))
    return result


def select_fields(row: dict) -> dict:
    """排除媒体、引用实例、词源引文；只保留选定词典字段及原标记。"""
    senses = []
    for sense in row.get("senses", []):
        if not isinstance(sense, dict):
            raise ValueError("invalid_source_sense")
        if any(tag in {"quotation", "quote", "copyrighted", "fair-use"} for tag in sense.get("tags", [])):
            raise ValueError("rights_or_quotation_marker")
        senses.append({key: sense[key] for key in ("glosses", "tags", "raw_tags", "topics", "categories") if key in sense})
    sounds = [sound for sound in row.get("sounds", []) if isinstance(sound, dict)
              and {"Mandarin", "Pinyin"} <= set(sound.get("tags", [])) and "zh_pron" in sound]
    translations = [{key: item[key] for key in ("lang_code", "word", "sense", "tags", "raw_tags") if key in item}
                    for item in row.get("translations", []) if isinstance(item, dict) and item.get("lang_code") == "en"]
    return {"word": row["word"], "lang_code": row["lang_code"], "pos": row.get("pos"),
            "pos_title": row.get("pos_title"), "senses": senses,
            "mandarin_pinyin": [{key: sound[key] for key in ("zh_pron", "tags", "raw_tags") if key in sound} for sound in sounds],
            "english_translations": translations, "rights_markers": rights_markers(row) + rights_markers(sounds, "/selected_pinyin")}


def import_words(source: Path, lock: dict, audit) -> list[dict]:
    records = []
    expanded = 0
    opener = source.open if lock["format"] == "wiktionary-zh-prefix-jsonl" else lambda mode: gzip.open(source, mode)
    with opener("rb") as stream:
        while raw := stream.readline(MAX_WORD_LINE + 1):
            expanded += len(raw)
            if len(raw) > MAX_WORD_LINE or expanded > MAX_WORD_EXPANDED:
                raise ValueError("词条原件单行或展开体积超限")
            audit.counts["raw_lines"] += 1
            line = audit.counts["raw_lines"]
            row = json.loads(raw.decode("utf-8"))
            if row.get("lang_code") != "zh":
                audit.counts["non_chinese_entries_skipped"] += 1
                continue
            word = row.get("word", "")
            if (not isinstance(word, str) or not 2 <= len(word) <= 16
                    or not all(0x3400 <= ord(char) <= 0x9FFF or 0x20000 <= ord(char) <= 0x3347F for char in word)):
                audit.counts["outside_multichar_han_scope_skipped"] += 1
                continue
            try:
                fields = select_fields(row)
            except ValueError as error:
                audit.reject(str(error), source.name, line, word)
                continue
            if fields["rights_markers"]:
                audit.reject("unreviewed_additional_rights_marker", source.name, line, canonical(fields["rights_markers"]).decode("utf-8").strip())
                continue
            sounds = fields["mandarin_pinyin"]
            if not sounds:
                audit.counts["missing_explicit_mandarin_pinyin"] += 1
                continue
            glosses = [value for sense in fields["senses"] for value in sense.get("glosses", [])]
            for reading in sorted({sound["zh_pron"] for sound in sounds}):
                try:
                    pinyin = normalize_word_reading(reading)
                    key = "wiki:" + digest(canonical([word, fields, reading]))[:32]
                    record = make_record(lock, key, word, pinyin, reading, glosses,
                                         [{"member": source.name, "line": line, "raw": reading, "raw_record_sha256": digest(raw)}],
                                         gloss_language="zh", source_fields=fields, source_record_sha256=digest(raw),
                                         attribution_url="https://zh.wiktionary.org/wiki/" + quote(word, safe=""),
                                         frequency_observation=None)
                    record["schema_version"] = 3
                    record["pos"] = fields["pos"]
                    records.append(record)
                except ValueError as error:
                    audit.reject(str(error), source.name, line, word + "\t" + reading)
    audit.counts["expanded_source_bytes"] = expanded
    return records


def validate_word_record(record: dict, lock: dict) -> None:
    common = {"schema_version", "id", "text", "pinyin", "raw_reading", "raw_senses", "pos", "frequency",
              "frequency_kind", "source", "source_version", "source_record_id", "license_id", "evidence"}
    extra = {"gloss_language", "source_fields", "source_record_sha256", "attribution_url", "frequency_observation"}
    if not isinstance(record, dict) or set(record) != common | extra or record["schema_version"] != 3:
        raise ValueError("词条事实schema错误")
    fields = record["source_fields"]
    if (not isinstance(fields, dict) or set(fields) != {"word", "lang_code", "pos", "pos_title", "senses", "mandarin_pinyin", "english_translations", "rights_markers"}
            or record["text"] != fields.get("word") or fields.get("lang_code") != "zh"
            or record["pos"] != fields.get("pos") or record["gloss_language"] != "zh"
            or record["frequency_observation"] is not None or record["frequency"] is not None
            or record["frequency_kind"] != "unknown" or fields.get("rights_markers") != []):
        raise ValueError("词条来源/语言/频率语义不一致")
    if fields["pos"] is not None and (not isinstance(fields["pos"], str) or not re.fullmatch("[A-Za-z-]{1,64}", fields["pos"])):
        raise ValueError("来源词性格式无效")
    if fields["pos_title"] is not None and (not isinstance(fields["pos_title"], str) or not clean_field(fields["pos_title"])):
        raise ValueError("来源词性名称无效")
    for name, allowed in [("senses", {"glosses", "tags", "raw_tags", "topics", "categories"}),
                          ("mandarin_pinyin", {"zh_pron", "tags", "raw_tags"}),
                          ("english_translations", {"lang_code", "word", "sense", "tags", "raw_tags"})]:
        if not isinstance(fields[name], list) or len(fields[name]) > 4096:
            raise ValueError("词条原始字段数量错误")
        for item in fields[name]:
            if not isinstance(item, dict) or not set(item) <= allowed:
                raise ValueError("词条原始字段格式错误")
            for value in item.values():
                values = value if isinstance(value, list) else [value]
                if any(not isinstance(text, str) or not clean_field(text) or len(text) > 65536 for text in values):
                    raise ValueError("词条原始字段字符串错误")
            if name == "mandarin_pinyin" and (not {"zh_pron", "tags"} <= set(item) or not isinstance(item["tags"], list)
                                             or not {"Mandarin", "Pinyin"} <= set(item["tags"])):
                raise ValueError("源读音未明确标识普通话拼音")
            if name == "english_translations" and (item.get("lang_code") != "en" or not item.get("word")):
                raise ValueError("英文翻译来源语言不符")
    if record["pinyin"] != normalize_word_reading(record["raw_reading"]):
        raise ValueError("词条读音投影不一致")
    if record["raw_reading"] not in {sound["zh_pron"] for sound in fields["mandarin_pinyin"]}:
        raise ValueError("词条缺少明确来源读音")
    if record["raw_senses"] != [value for sense in fields["senses"] for value in sense.get("glosses", [])]:
        raise ValueError("词条义项投影不一致")
    if (not isinstance(record["text"], str) or not clean_field(record["text"]) or not 2 <= len(record["text"]) <= 16
            or not all(0x3400 <= ord(char) <= 0x9FFF or 0x20000 <= ord(char) <= 0x3347F for char in record["text"])
            or any(not clean_field(value) for value in record["raw_senses"])):
        raise ValueError("词条字符串不合法")
    if record["attribution_url"] != "https://zh.wiktionary.org/wiki/" + quote(record["text"], safe=""):
        raise ValueError("逐页署名不一致")
    if not re.fullmatch("[a-f0-9]{64}", record["source_record_sha256"]):
        raise ValueError("源记录SHA错误")
    if (record["source"] != lock["source_id"] or record["source_version"] != lock["version"]
            or record["license_id"] != lock["license_id"]):
        raise ValueError("词条来源许可不一致")
    key = "wiki:" + digest(canonical([record["text"], fields, record["raw_reading"]]))[:32]
    expected_id = "entry:" + digest(canonical([record["source"], key, record["text"], record["pinyin"]]))[:32]
    if record["source_record_id"] != key or record["id"] != expected_id:
        raise ValueError("词条稳定ID不一致")
    if not isinstance(record["evidence"], list) or not record["evidence"]:
        raise ValueError("词条缺少行号证据")
    for evidence in record["evidence"]:
        if (set(evidence) != {"member", "line", "raw", "raw_record_sha256"} or evidence["raw"] != record["raw_reading"]
                or type(evidence["line"]) is not int or not 1 <= evidence["line"] <= 0xFFFFFFFF
                or not isinstance(evidence["member"], str) or Path(evidence["member"]).name != evidence["member"]
                or any(char in evidence["member"] for char in "/\\:")
                or not isinstance(evidence["raw_record_sha256"], str) or not re.fullmatch("[a-f0-9]{64}", evidence["raw_record_sha256"])):
            raise ValueError("词条行号证据错误")
