"""与Google SentencePiece0.2.1实际差分；不是中文翻译质量评测。"""
from pathlib import Path
import argparse
import hashlib
import json
import subprocess


def main():
    import sentencepiece
    if sentencepiece.__version__ != '0.2.1':
        raise ValueError('官方参考版本不符')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--samples', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    samples = [json.loads(line)['text'] for line in args.samples.read_text(encoding='utf-8').splitlines()]
    samples += ['', '  leading   spaces\t', 'ＡＢＣ１２３', 'café e\u0301', '你好🙂🚀', '第一行\n第二行', 'a\u00a0b\u3000c', '\r\n', '𠮷野家', '中文—English“quotes”']
    results = []
    args.output.mkdir(parents=True, exist_ok=False)
    for direction in ['en-zh', 'zh-en']:
        model = args.bundle / ('opus-mt-' + direction)
        command = [str(args.binary.resolve()), 'tokenize', str((model / 'source.spm').resolve()), str((model / 'target.spm').resolve()), str((model / 'vocab.json').resolve()), direction]
        encoded = ''.join(json.dumps(text, ensure_ascii=False) + '\n' for text in samples).encode('utf-8')
        result = subprocess.run(command, input=encoded, capture_output=True)
        (args.output / (direction + '-stdout.jsonl')).write_bytes(result.stdout)
        (args.output / (direction + '-stderr.txt')).write_bytes(result.stderr)
        if result.returncode:
            raise RuntimeError('真实tokenizer进程失败，原文已保留')
        native = [json.loads(line) for line in result.stdout.decode('utf-8').splitlines()]
        if len(native) != len(samples):
            raise ValueError('原始失败分母不符')
        source = sentencepiece.SentencePieceProcessor(model_file=str(model / 'source.spm'))
        target = sentencepiece.SentencePieceProcessor(model_file=str(model / 'target.spm'))
        vocab = json.loads((model / 'vocab.json').read_text(encoding='utf-8'))
        inverse = {value: key for key, value in vocab.items()}
        for index, text in enumerate(samples):
            pieces = source.encode(text, out_type=str)
            if direction == 'en-zh':
                pieces.insert(0, '>>cmn_Hans<<')
            pieces.append('</s>')
            ids = [vocab.get(piece, vocab['<unk>']) for piece in pieces]
            decoded = target.decode([inverse[token] for token in ids if token not in [0, 65000]])
            expected = {'pieces': pieces, 'ids': ids, 'decoded_using_target': decoded}
            results.append({'direction': direction, 'index': index, 'text': text, 'native': native[index], 'official': expected, 'identical': native[index] == expected})
    record = {'official_version': sentencepiece.__version__, 'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(),
              'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'cases': len(results),
              'matched': sum(row['identical'] for row in results), 'results': results}
    (args.output / 'comparison.json').write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps({key: value for key, value in record.items() if key != 'results'}))
    if record['matched'] != record['cases']:
        raise ValueError('官方SentencePiece差分存在不一致，不能报告等价')


if __name__ == '__main__':
    main()
