"""仅显式研究摘录允许未完成gzip前缀；完整包构建不使用此路径。"""

import hashlib
from pathlib import Path
import zlib


def extract_prefix(prefix: Path, expected_sha256: str, output: Path) -> dict:
    if output.exists() or prefix.stat().st_size > 512 * 1024 * 1024:
        raise ValueError("摘录输出已存在或压缩前缀超限")
    source_hash = hashlib.sha256()
    with prefix.open("rb") as stream:
        while block := stream.read(1024 * 1024):
            source_hash.update(block)
    if source_hash.hexdigest() != expected_sha256:
        raise ValueError("固定压缩前缀哈希不匹配")
    inflater = zlib.decompressobj(16 + zlib.MAX_WBITS)
    pending = b""
    expanded, written, lines = 0, 0, 0
    target_hash = hashlib.sha256()
    output.parent.mkdir(parents=True, exist_ok=True)
    try:
        with prefix.open("rb") as source, output.open("xb") as target:
            while block := source.read(64 * 1024):
                tail = block
                while tail:
                    value = inflater.decompress(tail, 1024 * 1024)
                    tail = inflater.unconsumed_tail
                    expanded += len(value)
                    if expanded > 4 * 1024 * 1024 * 1024:
                        raise ValueError("前缀展开超限")
                    pending += value
                    boundary = pending.rfind(b"\n") + 1
                    if boundary:
                        complete, pending = pending[:boundary], pending[boundary:]
                        complete.decode("utf-8")
                        target.write(complete)
                        target_hash.update(complete)
                        written += len(complete)
                        lines += complete.count(b"\n")
                    if len(pending) > 4 * 1024 * 1024:
                        raise ValueError("前缀单行超限")
            if inflater.eof or inflater.unused_data:
                raise ValueError("研究前缀意外包含完整gzip或尾随数据，需另行审核")
        return {"format": "wiktionary-zh-prefix-jsonl", "source_subset": True, "research_only": True,
                "prefix_sha256": expected_sha256, "prefix_bytes": prefix.stat().st_size,
                "compressed_range": [0, prefix.stat().st_size - 1], "original_sha256": None,
                "full_source_download_complete": False, "full_gzip_crc_verified": False,
                "sha256": target_hash.hexdigest(), "bytes": written, "line_count": lines,
                "decompressed_prefix_bytes": expanded, "discarded_incomplete_tail_bytes": len(pending),
                "derivation": "Inflate fixed compressed prefix; preserve complete original LF-terminated UTF-8 JSONL lines only; discard trailing incomplete line; no lexical/gold selection."}
    except BaseException:
        if output.exists():
            output.unlink()
        raise
