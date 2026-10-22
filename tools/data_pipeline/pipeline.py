"""有界离线导入、确定性资源构建和单写者原子发布。"""

from __future__ import annotations

from collections import Counter, defaultdict
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import tempfile
import unicodedata
import urllib.request
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[2]
VERSION = "cloudleaf-data-2.0.0"
HEADER = "id\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id"
MAX_SOURCE = 32 * 1024 * 1024
MAX_EXPANDED = 128 * 1024 * 1024
MAX_RELEASE = 256 * 1024 * 1024
IDENTIFIER = re.compile(r"[A-Za-z0-9_.:-]{1,128}\Z")
SHA256 = re.compile(r"[a-f0-9]{64}\Z")
# 与项目自编全拼表保持一致，表摘要随清单锁定；不从第三方输入法导入。
SYLLABLES = set("""
a ai an ang ao e ei en eng er o ou
ba bai ban bang bao bei ben beng bi bian biao bie bin bing bo bu
pa pai pan pang pao pei pen peng pi pian piao pie pin ping po pou pu
ma mai man mang mao me mei men meng mi mian miao mie min ming miu mo mou mu
fa fan fang fei fen feng fo fou fu
da dai dan dang dao de dei den deng di dia dian diao die ding diu dong dou du duan dui dun duo
ta tai tan tang tao te teng ti tian tiao tie ting tong tou tu tuan tui tun tuo
na nai nan nang nao ne nei nen neng ni nian niang niao nie nin ning niu nong nou nu nuan nuo nv nve
la lai lan lang lao le lei leng li lia lian liang liao lie lin ling liu lo long lou lu luan lun luo lv lve
ga gai gan gang gao ge gei gen geng gong gou gu gua guai guan guang gui gun guo
ka kai kan kang kao ke kei ken keng kong kou ku kua kuai kuan kuang kui kun kuo
ha hai han hang hao he hei hen heng hong hou hu hua huai huan huang hui hun huo
ji jia jian jiang jiao jie jin jing jiong jiu ju juan jue jun
qi qia qian qiang qiao qie qin qing qiong qiu qu quan que qun
xi xia xian xiang xiao xie xin xing xiong xiu xu xuan xue xun
zha zhai zhan zhang zhao zhe zhei zhen zheng zhi zhong zhou zhu zhua zhuai zhuan zhuang zhui zhun zhuo
cha chai chan chang chao che chen cheng chi chong chou chu chu chua chuai chuan chuang chui chun chuo
sha shai shan shang shao she shei shen sheng shi shou shu shua shuai shuan shuang shui shun shuo
ran rang rao re ren reng ri rong rou ruan rui run ruo ru
za zai zan zang zao ze zei zen zeng zi zong zou zu zuan zui zun zuo
ca cai can cang cao ce cen ceng ci cong cou cu cuan cui cun cuo
sa sai san sang sao se sen seng si song sou su suan sui sun suo
ya yan yang yao ye yi yin ying yo yong you yu yuan yue yun
wa wai wan wang wei wen weng wo wu
""".split())


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def builder_fingerprint() -> dict:
    return {path.name: digest(path.read_bytes()) for path in sorted(Path(__file__).parent.glob("*.py"))}


def write_json(path: Path, value: object) -> None:
    path.write_bytes(canonical(value))


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def clean_field(value: str, *, empty: bool = False) -> bool:
    return (empty or bool(value)) and value == value.strip() and not any(
        unicodedata.category(char) in {"Cc", "Cf", "Cs"} for char in value
    )


def normalize_reading(raw: str) -> str:
    """只移除声调，保留其他附加符号；不能表达的读音明确拒绝。"""
    raw = raw.lower().replace("u:", "ü").replace("v", "ü")
    raw = re.sub(r"[1-5]$", "", raw)
    value = "".join(char for char in unicodedata.normalize("NFD", raw) if char not in "\u0300\u0301\u0304\u030c")
    value = unicodedata.normalize("NFC", value).replace("ü", "v")
    if value.startswith(("jv", "qv", "xv", "yv")):
        value = value.replace("v", "u", 1)
    if value not in SYLLABLES:
        raise ValueError("unsupported_reading")
    return value


def load_lock(path: Path) -> dict:
    lock = read_json(path)
    if lock.get("schema_version") != 1 or lock.get("review_status") != "approved":
        raise ValueError("来源未审核通过")
    for field in ("source_id", "license_id"):
        if not IDENTIFIER.fullmatch(lock[field]):
            raise ValueError("来源或许可标识错误")
    if not SHA256.fullmatch(lock["sha256"]) or not SHA256.fullmatch(lock["license_sha256"]):
        raise ValueError("必须先锁定原件与许可SHA-256")
    if lock["format"] not in {"unihan-zip", "cedict-text"}:
        raise ValueError("来源格式不支持")
    return lock


def fetch(lock_path: Path, output: Path) -> str:
    """仅显式取得已批准且允许自动下载的固定HTTPS原件。"""
    lock = load_lock(lock_path)
    if not lock.get("automated_download_allowed") or not lock["url"].startswith("https://"):
        raise ValueError("来源未允许自动取得；请使用已审核本地原件")
    if output.exists():
        if digest(output.read_bytes()) != lock["sha256"]:
            raise ValueError("缓存原件哈希不匹配")
        return lock["sha256"]
    output.parent.mkdir(parents=True, exist_ok=True)
    request = urllib.request.Request(lock["url"], headers={"User-Agent": VERSION})
    with urllib.request.urlopen(request, timeout=30) as response:
        if response.geturl() != lock["url"]:
            raise ValueError("来源发生重定向，需重新审核URL")
        data = response.read(MAX_SOURCE + 1)
    if len(data) > MAX_SOURCE or digest(data) != lock["sha256"]:
        raise ValueError("原件体积或哈希不匹配")
    # 排他写入，不替换不同原件。
    with output.open("xb") as stream:
        stream.write(data)
    return lock["sha256"]


class Audit:
    def __init__(self) -> None:
        self.counts: Counter = Counter({key: 0 for key in (
            "raw_lines", "selected_property_lines", "duplicate_property_lines", "duplicate_records",
            "rejected_records", "conflicting_characters", "input_reading_records", "characters_with_mandarin")})
        self.rejections: list[dict] = []

    def reject(self, reason: str, member: str, line: int, raw: str) -> None:
        self.counts["rejected_records"] += 1
        self.rejections.append({"reason": reason, "member": member, "line": line, "raw": raw})


def make_record(lock: dict, source_record_id: str, text: str, pinyin: list[str], raw_reading: str,
                senses: list[str], evidence: list[dict], **extra: object) -> dict:
    if (not clean_field(text) or len(text) > 64 or not (1 <= len(pinyin) <= 16)
            or sum(map(len, pinyin)) > 64):
        raise ValueError("invalid_text_or_syllable_count")
    if not all(clean_field(sense) for sense in senses):
        raise ValueError("invalid_sense")
    key = canonical([lock["source_id"], source_record_id, text, pinyin])
    return {"schema_version": 2, "id": "entry:" + digest(key)[:32], "text": text, "pinyin": pinyin,
            "raw_reading": raw_reading, "raw_senses": senses, "pos": None,
            "frequency": None, "frequency_kind": "unknown", "source": lock["source_id"],
            "source_version": lock["version"], "source_record_id": source_record_id,
            "license_id": lock["license_id"], "evidence": evidence, **extra}


def import_unihan(source: Path, lock: dict, audit: Audit) -> list[dict]:
    properties: dict[str, dict[str, tuple[str, dict]]] = defaultdict(dict)
    conflicts = set()
    with ZipFile(source) as archive:
        infos = sorted(archive.infolist(), key=lambda item: item.filename)
        if len(infos) > 64 or sum(info.file_size for info in infos) > MAX_EXPANDED:
            raise ValueError("归档展开体积超限")
        names = set()
        for info in infos:
            member = PurePosixPath(info.filename)
            if (info.filename in names or len(member.parts) != 1 or "\\" in info.filename
                    or member.is_absolute() or not re.fullmatch(r"Unihan_[A-Za-z]+\.txt", info.filename)
                    or (info.external_attr >> 16) & 0o170000 == 0o120000):
                raise ValueError("归档成员路径、类型或重复名称无效")
            names.add(info.filename)
            content = archive.read(info)
            if len(content) != info.file_size:
                raise ValueError("归档成员体积不匹配")
            text = content.decode("utf-8")
            if text.startswith("\ufeff"):
                raise ValueError("来源成员包含BOM")
            for number, line in enumerate(text.splitlines(), 1):
                audit.counts["raw_lines"] += 1
                if not line or line.startswith("#"):
                    continue
                fields = line.split("\t")
                if len(fields) != 3:
                    audit.reject("field_count", info.filename, number, line)
                    continue
                code, prop, value = fields
                if prop not in {"kMandarin", "kDefinition"}:
                    continue
                audit.counts["selected_property_lines"] += 1
                if not re.fullmatch(r"U\+[0-9A-F]{4,6}", code) or not clean_field(value):
                    audit.reject("invalid_property", info.filename, number, line)
                    continue
                scalar = int(code[2:], 16)
                if scalar > 0x10FFFF or 0xD800 <= scalar <= 0xDFFF:
                    audit.reject("invalid_scalar", info.filename, number, line)
                    continue
                evidence = {"member": info.filename, "line": number, "property": prop, "raw": value}
                old = properties[code].get(prop)
                if old:
                    if old[0] == value:
                        audit.counts["duplicate_property_lines"] += 1
                    else:
                        conflicts.add(code)
                        audit.reject("source_property_conflict", info.filename, number, line)
                    continue
                properties[code][prop] = value, evidence
    records = []
    audit.counts["source_characters"] = len(properties)
    audit.counts["conflicting_characters"] = len(conflicts)
    for code, props in sorted(properties.items(), key=lambda pair: int(pair[0][2:], 16)):
        if code in conflicts:
            continue
        if "kMandarin" not in props:
            audit.counts["characters_without_mandarin"] += 1
            continue
        reading, evidence = props["kMandarin"]
        audit.counts["characters_with_mandarin"] += 1
        definition = props.get("kDefinition")
        for ordinal, raw in enumerate(reading.split(" ")):
            audit.counts["input_reading_records"] += 1
            try:
                pinyin = [normalize_reading(raw)]
                record = make_record(lock, f"U{code[2:]}:{ordinal}", chr(int(code[2:], 16)), pinyin, raw,
                                     [definition[0]] if definition else [],
                                     [evidence] + ([definition[1]] if definition else []),
                                     reading_preference="shared" if " " not in reading else ["zh-Hans", "zh-Hant"][ordinal],
                                     raw_properties={key: value[0] for key, value in props.items()})
                records.append(record)
            except (ValueError, IndexError) as error:
                audit.reject(str(error), evidence["member"], evidence["line"], f"{code}\tkMandarin\t{raw}")
    return records


def import_cedict(source: Path, lock: dict, audit: Audit) -> list[dict]:
    """只读本地授权原件，不请求CC-CEDICT官网；不推断词性或统计频率。"""
    records = []
    for number, line in enumerate(source.read_bytes().decode("utf-8").splitlines(), 1):
        audit.counts["raw_lines"] += 1
        if not line or line.startswith("#"):
            continue
        match = re.fullmatch(r"([^ ]+) ([^ ]+) \[([^\[\]]+)\] /(.+)/", line)
        if not match:
            audit.reject("cedict_syntax", source.name, number, line)
            continue
        traditional, simplified, raw_reading, meanings = match.groups()
        try:
            pinyin = [normalize_reading(token) for token in raw_reading.split(" ")]
            record_key = digest(canonical([traditional, simplified, raw_reading, meanings]))[:32]
            records.append(make_record(lock, "cedict:" + record_key, simplified, pinyin, raw_reading,
                                       meanings.split("/"), [{"member": source.name, "line": number, "raw": line}],
                                       traditional=traditional, simplified=simplified))
        except ValueError as error:
            audit.reject(str(error), source.name, number, line)
    return records


def runtime(records: list[dict]) -> bytes:
    rows = ["# cloudleaf-lexicon-v2", HEADER]
    for record in records:
        # 长释义留在富记录；投影超长时明确不显示，不截断伪造完整释义。
        annotation = "; ".join(record["raw_senses"])
        annotation = annotation if len(annotation) <= 256 else ""
        fields = [record["id"], " ".join(record["pinyin"]), record["text"], "", "unknown", annotation,
                  record["source"], record["source_record_id"], record["license_id"]]
        if not all(clean_field(field, empty=True) for field in fields):
            raise ValueError("运行时字段无法无损表达")
        rows.append("\t".join(fields))
    return ("\n".join(rows) + "\n").encode("utf-8")


def build(lock_path: Path, source: Path, license_path: Path, output: Path) -> dict:
    lock = load_lock(lock_path)
    if output.exists():
        raise ValueError("输出目录已存在，拒绝覆盖")
    if source.stat().st_size > MAX_SOURCE or digest(source.read_bytes()) != lock["sha256"]:
        raise ValueError("原件体积或SHA-256不匹配")
    license_data = license_path.read_bytes()
    if digest(license_data) != lock["license_sha256"]:
        raise ValueError("许可证原文SHA-256不匹配")
    license_data.decode("utf-8")
    audit = Audit()
    importer = import_unihan if lock["format"] == "unihan-zip" else import_cedict
    raw_records = importer(source, lock, audit)
    unique = {}
    for record in raw_records:
        key = record["id"]
        if key in unique:
            audit.counts["duplicate_records"] += 1
            unique[key]["evidence"].extend(record["evidence"])
        else:
            unique[key] = record
    records = sorted(unique.values(), key=lambda record: record["id"])
    if not records:
        raise ValueError("没有可构建记录；拒绝生成空release")
    by_pinyin, by_text = defaultdict(list), defaultdict(list)
    for record in records:
        by_pinyin[" ".join(record["pinyin"])].append(record["id"])
        by_text[record["text"]].append(record["id"])
    audit.counts.update({"accepted_records": len(records), "distinct_texts": len(by_text),
                         "distinct_pinyin": len(by_pinyin), "missing_frequency": len(records),
                         "missing_pos": len(records), "missing_english_gloss": sum(not row["raw_senses"] for row in records),
                         "runtime_annotation_omitted_long": sum(len("; ".join(row["raw_senses"])) > 256 for row in records)})
    audit.counts["unsupported_runtime_readings"] = sum(row["reason"] == "unsupported_reading" for row in audit.rejections)
    audit.counts["malformed_or_conflicting_records"] = len(audit.rejections) - audit.counts["unsupported_runtime_readings"]
    quality = {"schema_version": 2, "source": lock["source_id"], "counts": dict(sorted(audit.counts.items())),
               "rejection_reasons": dict(sorted(Counter(row["reason"] for row in audit.rejections).items())),
               "scope": lock["limitations"], "evaluation": {"train": None, "dev": None, "heldout": None,
               "accuracy_claim": False, "reason": "来源数据完整性检查，不是独立语言评测"},
               "source_conflicts": "Unihan重复属性冲突字符全部隔离，不择一覆盖"}
    output.parent.mkdir(parents=True, exist_ok=True)
    # 临时目录仅在指定输出的父目录内创建，失败不会改变已有release。
    with tempfile.TemporaryDirectory(prefix=".cloudleaf-build-", dir=output.parent) as temporary:
        stage = Path(temporary)
        (stage / "records.jsonl").write_bytes(b"".join(canonical(record) for record in records))
        (stage / "runtime.tsv").write_bytes(runtime(records))
        (stage / "rejected.jsonl").write_bytes(b"".join(canonical(row) for row in audit.rejections))
        write_json(stage / "index.json", {"schema_version": 2, "by_pinyin": by_pinyin, "by_text": by_text})
        write_json(stage / "quality.json", quality)
        write_json(stage / "source-lock.json", lock)
        (stage / "LICENSE.txt").write_bytes(license_data)
        notice = (f"{lock['attribution']}\nSource: {lock['url']}\nLicense: {lock['license_id']} {lock['license_url']}\n"
                  f"Changes: {lock['transformations']}\nNo frequency or part-of-speech inference.\n")
        (stage / "NOTICE.txt").write_bytes(notice.encode("utf-8"))
        resources = {path.name: {"sha256": digest(path.read_bytes()), "bytes": path.stat().st_size}
                     for path in sorted(stage.iterdir())}
        manifest = {"schema_version": 2, "runtime_format": "cloudleaf-lexicon-v2", "builder": VERSION,
                    "builder_files_sha256": builder_fingerprint(),
                    "normalization_unicode_version": unicodedata.unidata_version,
                    "syllables_sha256": digest(canonical(sorted(SYLLABLES))),
                    "source": lock, "source_lock_sha256": digest(canonical(lock)),
                    "entry_count": len(records), "rejected_count": len(audit.rejections),
                    "resources": resources, "license_file": "LICENSE.txt"}
        write_json(stage / "manifest.json", manifest)
        verify_release(stage)
        # 同卷rename作为完成标记，目录存在时不会覆盖已有结果。
        stage.rename(output)
    return manifest


def safe_child(root: Path, name: str) -> Path:
    if not name or Path(name).name != name or "/" in name or "\\" in name or ":" in name or name in {".", ".."}:
        raise ValueError("资源路径越界")
    path = root / name
    if path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
        raise ValueError("资源路径含链接或越界")
    return path


def validate_record(record: dict, lock: dict) -> None:
    """校验事实层语义，不能以重新生成哈希绕过格式/来源约束。"""
    common = {"schema_version", "id", "text", "pinyin", "raw_reading", "raw_senses", "pos", "frequency",
              "frequency_kind", "source", "source_version", "source_record_id", "license_id", "evidence"}
    extra = {"reading_preference", "raw_properties"} if lock["format"] == "unihan-zip" else {"traditional", "simplified"}
    if not isinstance(record, dict) or set(record) != common | extra or record["schema_version"] != 2:
        raise ValueError("事实记录schema错误")
    if record["source_version"] != lock["version"] or record["source"] != lock["source_id"] or record["license_id"] != lock["license_id"]:
        raise ValueError("事实记录来源版本或许可映射错误")
    for field in ("id", "source", "source_record_id", "license_id"):
        if not isinstance(record[field], str) or not IDENTIFIER.fullmatch(record[field]):
            raise ValueError("记录标识不合法")
    if not isinstance(record["text"], str) or not clean_field(record["text"]) or len(record["text"]) > 64:
        raise ValueError("事实记录text不合法")
    pinyin = record["pinyin"]
    if (not isinstance(pinyin, list) or not 1 <= len(pinyin) <= 16
            or any(not isinstance(value, str) or value not in SYLLABLES for value in pinyin)
            or sum(map(len, pinyin)) > 64):
        raise ValueError("拼音音节或预算错误")
    raw = record["raw_reading"]
    if not isinstance(raw, str) or len(raw) > 256 or not clean_field(raw):
        raise ValueError("原始读音不合法")
    if [normalize_reading(value) for value in raw.split(" ")] != pinyin:
        raise ValueError("原始读音与规范拼音不一致")
    senses = record["raw_senses"]
    if (not isinstance(senses, list) or len(senses) > 1024
            or any(not isinstance(value, str) or len(value) > 65536 or not clean_field(value) for value in senses)):
        raise ValueError("原始义项不合法")
    if record["frequency"] is not None or record["frequency_kind"] != "unknown" or record["pos"] is not None:
        raise ValueError("本轮来源不能生成词频或词性")
    evidence = record["evidence"]
    if not isinstance(evidence, list) or not 1 <= len(evidence) <= 4096:
        raise ValueError("来源证据数量不合法")
    for item in evidence:
        expected = {"member", "line", "raw", "property"} if lock["format"] == "unihan-zip" else {"member", "line", "raw"}
        if (not isinstance(item, dict) or set(item) != expected or type(item["line"]) is not int
                or not 1 <= item["line"] <= 0xFFFFFFFF or not isinstance(item["member"], str)
                or len(item["member"]) > 256 or not clean_field(item["member"])
                or Path(item["member"]).name != item["member"] or "/" in item["member"] or "\\" in item["member"]
                or not isinstance(item["raw"], str) or len(item["raw"]) > 65536 or not clean_field(item["raw"])):
            raise ValueError("来源证据结构不合法")
        if lock["format"] == "unihan-zip" and item["property"] not in {"kMandarin", "kDefinition"}:
            raise ValueError("来源证据属性不合法")
    if lock["format"] == "unihan-zip":
        props = record["raw_properties"]
        if (not isinstance(props, dict) or not {"kMandarin"} <= set(props) <= {"kMandarin", "kDefinition"}
                or any(not isinstance(value, str) or not clean_field(value) or len(value) > 65536 for value in props.values())
                or record["raw_reading"] not in props["kMandarin"].split(" ")
                or senses != ([props["kDefinition"]] if "kDefinition" in props else [])):
            raise ValueError("Unihan原始属性或义项不一致")
        match = re.fullmatch(r"U([0-9A-F]{4,6}):([01])", record["source_record_id"])
        if not match or chr(int(match[1], 16)) != record["text"]:
            raise ValueError("Unihan来源记录ID不匹配")
        readings = props["kMandarin"].split(" ")
        if not 1 <= len(readings) <= 2 or any(item["raw"] != props.get(item["property"]) for item in evidence):
            raise ValueError("Unihan证据与原始属性不一致")
        if {item["property"] for item in evidence} != set(props):
            raise ValueError("Unihan属性缺失来源证据")
        ordinal = int(match[2])
        if ordinal >= len(readings) or readings[ordinal] != raw:
            raise ValueError("Unihan来源读音位置不匹配")
        preference = "shared" if len(readings) == 1 else ["zh-Hans", "zh-Hant"][ordinal]
        if record["reading_preference"] != preference:
            raise ValueError("Unihan读音地区优先级不匹配")
    else:
        if any(not isinstance(record[field], str) or not clean_field(record[field]) or len(record[field]) > 64
               for field in ("traditional", "simplified")) or record["text"] != record["simplified"]:
            raise ValueError("CC-CEDICT字形不合法")
        key = digest(canonical([record["traditional"], record["simplified"], raw, "/".join(senses)]))[:32]
        if record["source_record_id"] != "cedict:" + key:
            raise ValueError("CC-CEDICT稳定记录ID不匹配")
        expected_raw = f"{record['traditional']} {record['simplified']} [{raw}] /{'/'.join(senses)}/"
        if any(item["raw"] != expected_raw for item in evidence):
            raise ValueError("CC-CEDICT来源证据与字段不一致")
    expected_id = "entry:" + digest(canonical([record["source"], record["source_record_id"], record["text"], pinyin]))[:32]
    if record["id"] != expected_id:
        raise ValueError("事实记录稳定ID重算不匹配")


def verify_release(root: Path) -> dict:
    if root.is_symlink():
        raise ValueError("release不可是符号链接")
    manifest_path = safe_child(root, "manifest.json")
    if manifest_path.stat().st_size > 1024 * 1024:
        raise ValueError("manifest体积超限")
    manifest = read_json(manifest_path)
    required = {"records.jsonl", "runtime.tsv", "index.json", "quality.json", "rejected.jsonl", "source-lock.json", "LICENSE.txt", "NOTICE.txt"}
    if (manifest.get("schema_version") != 2 or manifest.get("runtime_format") != "cloudleaf-lexicon-v2"
            or set(manifest["resources"]) != required
            or {path.name for path in root.iterdir()} != required | {"manifest.json"}):
        raise ValueError("release版本、资源清单或未知文件不匹配")
    if (sum(row["bytes"] for row in manifest["resources"].values()) > MAX_RELEASE
            or any(type(row["bytes"]) is not int or not 0 <= row["bytes"] <= MAX_EXPANDED
                   or not isinstance(row["sha256"], str) or not SHA256.fullmatch(row["sha256"])
                   for row in manifest["resources"].values())):
        raise ValueError("release资源体积或哈希格式超限")
    for name, expected in manifest["resources"].items():
        path = safe_child(root, name)
        if path.stat().st_size != expected["bytes"]:
            raise ValueError(f"资源哈希或体积不匹配：{name}")
        data = path.read_bytes()
        if len(data) != expected["bytes"] or digest(data) != expected["sha256"]:
            raise ValueError(f"资源哈希或体积不匹配：{name}")
        text = data.decode("utf-8")
        if text.startswith("\ufeff"):
            raise ValueError("资源不接受BOM")
    lock = load_lock(root / "source-lock.json")
    if lock != manifest["source"] or digest(canonical(lock)) != manifest["source_lock_sha256"]:
        raise ValueError("来源锁不匹配")
    if digest((root / "LICENSE.txt").read_bytes()) != lock["license_sha256"]:
        raise ValueError("许可映射不匹配")
    records = [json.loads(line) for line in (root / "records.jsonl").read_text(encoding="utf-8").splitlines()]
    if type(manifest["entry_count"]) is not int or len(records) != manifest["entry_count"] or not records:
        raise ValueError("记录数不匹配")
    ids = set()
    expected_pinyin, expected_text = defaultdict(list), defaultdict(list)
    for record in records:
        validate_record(record, lock)
        if record["id"] in ids or record["source"] != lock["source_id"] or record["license_id"] != lock["license_id"]:
            raise ValueError("重复ID或来源许可映射错误")
        ids.add(record["id"])
        if record["frequency"] is not None or record["frequency_kind"] != "unknown" or record["pos"] is not None:
            raise ValueError("本轮来源不能生成词频或词性")
        if not record["pinyin"] or any(value not in SYLLABLES for value in record["pinyin"]):
            raise ValueError("拼音音节错误")
        expected_pinyin[" ".join(record["pinyin"])].append(record["id"])
        expected_text[record["text"]].append(record["id"])
    if [record["id"] for record in records] != sorted(ids):
        raise ValueError("事实记录排序不确定")
    if (root / "runtime.tsv").read_bytes() != runtime(records):
        raise ValueError("运行时投影与事实层不一致")
    index = read_json(root / "index.json")
    if index != {"schema_version": 2, "by_pinyin": expected_pinyin, "by_text": expected_text}:
        raise ValueError("索引不一致")
    quality = read_json(root / "quality.json")
    rejected = (root / "rejected.jsonl").read_text(encoding="utf-8").splitlines()
    if (quality["counts"]["accepted_records"] != len(records) or manifest["rejected_count"] != len(rejected)
            or quality["counts"].get("rejected_records", 0) != len(rejected)):
        raise ValueError("质量报告计数不一致")
    return manifest


@contextmanager
def publish_lock(store: Path):
    store.mkdir(parents=True, exist_ok=True)
    path = store / ".writer.lock"
    try:
        stream = path.open("x", encoding="utf-8")
    except FileExistsError as error:
        raise ValueError("已有发布写者；崩溃遗留锁需人工确认后移除") from error
    try:
        with stream:
            stream.write(str(os.getpid()))
            stream.flush()
        yield
    finally:
        path.unlink()


def atomic_json(path: Path, value: dict) -> None:
    temporary = path.with_suffix(".tmp")
    with temporary.open("wb") as stream:
        stream.write(canonical(value))
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def checked_pointer(store: Path, pointer: dict) -> None:
    release = safe_child(store / "releases", pointer["release"])
    verify_release(release)
    if digest((release / "manifest.json").read_bytes()) != pointer["manifest_sha256"]:
        raise ValueError("发布指针哈希错误")


def activate(release: Path, store: Path) -> dict:
    verify_release(release)
    release_hash = digest((release / "manifest.json").read_bytes())
    with publish_lock(store):
        current = read_json(store / "current.json") if (store / "current.json").exists() else None
        if current:
            if current.get("schema_version") != 1:
                raise ValueError("发布指针版本错误")
            checked_pointer(store, current["current"])
        releases = store / "releases"
        releases.mkdir(exist_ok=True)
        target = releases / release_hash
        if not target.exists():
            with tempfile.TemporaryDirectory(prefix=".publish-", dir=store) as temporary:
                staging = Path(temporary) / "release"
                shutil.copytree(release, staging)
                verify_release(staging)
                if digest((staging / "manifest.json").read_bytes()) != release_hash:
                    raise ValueError("复制的发布包清单与请求哈希不一致")
                staging.rename(target)
        verify_release(target)
        if digest((target / "manifest.json").read_bytes()) != release_hash:
            raise ValueError("缓存发布包清单与请求哈希不一致")
        pointer = {"release": release_hash, "manifest_sha256": release_hash}
        if current and current["current"] == pointer:
            return current
        state = {"schema_version": 1, "current": pointer, "previous": current["current"] if current else None}
        atomic_json(store / "current.json", state)
        return state


def rollback(store: Path) -> dict:
    with publish_lock(store):
        current = read_json(store / "current.json")
        if current.get("schema_version") != 1 or not current.get("previous"):
            raise ValueError("没有兼容的上一版本")
        # 当前版本可以损坏；回退目标必须完整且与指针一致。
        checked_pointer(store, current["previous"])
        state = {"schema_version": 1, "current": current["previous"], "previous": current["current"]}
        atomic_json(store / "current.json", state)
        return state
