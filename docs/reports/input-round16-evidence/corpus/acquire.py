from pathlib import Path
import urllib.request
import hashlib
import json
import bz2
import datetime

ROOT = Path(__file__).resolve().parent
URLS = {
    'cmn_sentences_detailed.tsv.bz2': 'https://downloads.tatoeba.org/exports/per_language/cmn/cmn_sentences_detailed.tsv.bz2',
    'cmn_sentences_base.tsv.bz2': 'https://downloads.tatoeba.org/exports/per_language/cmn/cmn_sentences_base.tsv.bz2',
    'cmn_sentences_CC0.tsv.bz2': 'https://downloads.tatoeba.org/exports/per_language/cmn/cmn_sentences_CC0.tsv.bz2',
    'cmn_tags.tsv.bz2': 'https://downloads.tatoeba.org/exports/per_language/cmn/cmn_tags.tsv.bz2',
    'cmn-cmn_links.tsv.bz2': 'https://downloads.tatoeba.org/exports/per_language/cmn/cmn-cmn_links.tsv.bz2',
    'downloads.html': 'https://tatoeba.org/en/downloads',
    'directory.html': 'https://downloads.tatoeba.org/exports/per_language/cmn/',
    'using-corpus.html': 'https://en.wiki.tatoeba.org/articles/show/using-the-tatoeba-corpus',
    'faq.html': 'https://en.wiki.tatoeba.org/articles/show/faq',
    'CC-BY-2.0-FR-legalcode.html': 'https://creativecommons.org/licenses/by/2.0/fr/legalcode',
    'terms.html': 'https://tatoeba.org/en/terms_of_use',
}

records = []
for filename, url in URLS.items():
    destination = ROOT / filename
    record = {'filename': filename, 'url': url, 'retrieved_utc': datetime.datetime.now(datetime.timezone.utc).isoformat()}
    try:
        if destination.exists():
            raise FileExistsError(str(destination))
        request = urllib.request.Request(url, headers={'User-Agent': 'Cloudleaf-Round16-research/1.0'})
        with urllib.request.urlopen(request, timeout=50) as response:
            payload = response.read(20_000_001)
            if len(payload) > 20_000_000:
                raise ValueError('source larger than the declared bounded download')
            record.update(status=response.status, final_url=response.url, headers={k: v for k, v in response.headers.items() if k.lower() != 'set-cookie'})
        destination.write_bytes(payload)
        record.update(bytes=len(payload), sha256=hashlib.sha256(payload).hexdigest())
        if filename.endswith('.bz2'):
            text = bz2.decompress(payload)
            text.decode('utf-8', errors='strict')
            (ROOT / filename[:-4]).write_bytes(text)
            record.update(bzip2_crc='passed_complete_eof', uncompressed_bytes=len(text), uncompressed_sha256=hashlib.sha256(text).hexdigest(), rows=len(text.splitlines()))
    except Exception as exc:
        record.update(error=f'{type(exc).__name__}: {exc}')
    records.append(record)
    (ROOT / 'acquisition.json').write_text(json.dumps(records, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(record, ensure_ascii=False))
