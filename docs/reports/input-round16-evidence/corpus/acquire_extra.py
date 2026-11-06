from pathlib import Path
import urllib.request
import hashlib
import json
import bz2
import datetime

ROOT = Path(__file__).resolve().parent
urls = {
    'cmn-eng_links.tsv.bz2': 'https://downloads.tatoeba.org/exports/per_language/cmn/cmn-eng_links.tsv.bz2',
    'guidelines.html': 'https://en.wiki.tatoeba.org/articles/show/guidelines',
    'CC-BY-2.0-FR-deed.html': 'https://creativecommons.org/licenses/by/2.0/fr/deed.en',
    'UD-licensing.html': 'https://universaldependencies.org/contributing/licensing.html',
}
records = []
for name, url in urls.items():
    record = {'filename': name, 'url': url, 'retrieved_utc': datetime.datetime.now(datetime.timezone.utc).isoformat()}
    try:
        p = ROOT / name
        if p.exists(): raise FileExistsError(str(p))
        with urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent': 'Cloudleaf-Round16-research/1.0'}), timeout=50) as response:
            raw = response.read(5_000_001)
            if len(raw) > 5_000_000: raise ValueError('source exceeds declared bound')
            record.update(status=response.status, final_url=response.url, headers={k:v for k,v in response.headers.items() if k.lower() != 'set-cookie'})
        p.write_bytes(raw)
        record.update(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())
        if name.endswith('.bz2'):
            uncompressed = bz2.decompress(raw)
            uncompressed.decode('utf-8', errors='strict')
            (ROOT / name[:-4]).write_bytes(uncompressed)
            record.update(bzip2_crc='passed_complete_eof', uncompressed_bytes=len(uncompressed), uncompressed_sha256=hashlib.sha256(uncompressed).hexdigest(), rows=len(uncompressed.splitlines()))
    except Exception as exc:
        record['error'] = f'{type(exc).__name__}: {exc}'
    records.append(record)
    (ROOT / 'extra-acquisition.json').write_text(json.dumps(records, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(record, ensure_ascii=False))
