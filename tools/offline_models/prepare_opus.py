"""从显式给定的固定原件构建研发模型目录；不联网、不转换权重、不覆盖目录。"""
from pathlib import Path
import argparse
import hashlib
import json
import shutil

ROOT = Path(__file__).resolve().parents[2]


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as file:
        while chunk := file.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--originals', type=Path, required=True)
    parser.add_argument('--source-files', type=Path, required=True)
    parser.add_argument('--converted', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    lock = json.loads((ROOT / 'config/offline-models/opus-source-lock.json').read_text(encoding='utf-8'))
    converted_lock = json.loads((ROOT / 'config/offline-models/opus-converted-lock.json').read_text(encoding='utf-8'))
    args.output.mkdir(parents=True, exist_ok=False)
    references = []
    for model in lock['models']:
        directory = args.output / model['model_id']
        directory.mkdir()
        files = []
        for row in model['files']:
            base = args.originals if row['role'] == 'weights' else args.source_files
            source = base / row['source_path']
            if source.stat().st_size != row['bytes'] or sha(source) != row['sha256']:
                raise ValueError(f'固定原件不符：{row["role"]}')
            target = directory / row['path']
            if row['role'] == 'weights':
                converted = next(item for item in converted_lock['models'] if item['model_id'] == model['model_id'])
                if converted['original_sha256'] != row['sha256']:
                    raise ValueError('转换来源与固定原件不一致')
                source = args.converted / model['model_id'] / converted['file']
                if source.stat().st_size != converted['bytes'] or sha(source) != converted['sha256']:
                    raise ValueError('转换权重SHA不一致')
                target = directory / converted['file']
                row = {'role': 'weights', 'path': converted['file'], 'bytes': converted['bytes'], 'sha256': converted['sha256']}
            shutil.copyfile(source, target)
            files.append({key: row[key] for key in ['role', 'path', 'bytes', 'sha256']})
        notice = (f'Cloudleaf local research model: {model["model_id"]}\n'
                  f'Publisher: Helsinki-NLP / OPUS-MT contributors. Source: {model["source_url"]}\n'
                  f'Revision: {model["revision"]}. Tokenizer bytes unchanged; weights converted to Safetensors.\n'
                  'Conversion preserved every retained tensor dtype/shape/bitwise value; tied embedding aliases removed only after equality checks.\n'
                  'Upstream model notice: CC BY 4.0; https://creativecommons.org/licenses/by/4.0/\n'
                  'https://github.com/Helsinki-NLP/OPUS-MT-train/blob/d5e932c96d77596378010aa7e7f1caeb8ccaa9f8/README.md\n'
                  'en-zh HF card separately declares Apache-2.0; this unresolved scope difference is retained.\n'
                  'Full training-data rights are unknown. Production model redistribution is not approved.\n'
                  'Candle CPU greedy execution is an engineering choice; formal quality is not accepted.\n')
        notice_path = directory / 'NOTICE.txt'
        notice_path.write_text(notice, encoding='utf-8', newline='\n')
        files.append({'role': 'notice', 'path': 'NOTICE.txt', 'bytes': notice_path.stat().st_size, 'sha256': sha(notice_path)})
        manifest = {key: model[key] for key in ['model_id', 'direction', 'source_url', 'revision']}
        manifest.update({'schema': 1, 'architecture': 'marian', 'runtime': 'candle-0.9.2-cpu',
                         'tokenizer': 'sentencepiece-rs-0.2.2', 'license_id': 'CC-BY-4.0',
                         'license_status': 'research_only', 'files': files})
        path = directory / 'manifest.json'
        path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
        references.append({'manifest': path.relative_to(args.output).as_posix(), 'sha256': sha(path)})
    catalog = args.output / 'catalog.json'
    catalog.write_text(json.dumps({'schema': 1, 'models': references}, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps({'catalog': str(catalog.resolve()), 'sha256': sha(catalog), 'models': len(references), 'status': 'checking_required'}))


if __name__ == '__main__':
    main()
