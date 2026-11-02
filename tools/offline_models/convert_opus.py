"""仅转换两个已核旧Torch原件；数值逐字节读回校验，不执行任意pickle。"""
from pathlib import Path
import argparse
import ctypes
import hashlib
import importlib.metadata
import json

ROOT = Path(__file__).resolve().parents[2]


def sha_file(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def tensor_identity(tensor):
    value = tensor.detach().cpu().contiguous()
    raw = ctypes.string_at(value.data_ptr(), value.numel() * value.element_size())
    return {'dtype': str(value.dtype), 'shape': list(value.shape), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}


def main():
    import torch
    from safetensors.torch import save_file, load_file
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--originals', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if torch.__version__ != '2.10.0+cpu' or importlib.metadata.version('safetensors') != '0.7.0':
        raise ValueError('转换工具版本不符')
    lock = json.loads((ROOT / 'config/offline-models/opus-source-lock.json').read_text(encoding='utf-8'))
    args.output.mkdir(parents=True, exist_ok=False)
    reports = []
    for model in lock['models']:
        original = next(row for row in model['files'] if row['role'] == 'weights')
        path = args.originals / original['source_path']
        if path.stat().st_size != original['bytes'] or sha_file(path) != original['sha256']:
            raise ValueError('源权重SHA不符，拒绝反序列化')
        state = torch.load(path, map_location='cpu', weights_only=True)
        shared = tensor_identity(state['model.shared.weight'])
        aliases = {}
        for name in ['model.encoder.embed_tokens.weight', 'model.decoder.embed_tokens.weight', 'lm_head.weight']:
            if name in state:
                identity = tensor_identity(state[name])
                if identity != shared:
                    raise ValueError('模型共享嵌入不一致，不能省略别名')
                aliases[name] = identity
        tensors = {key: value.detach().cpu().contiguous() for key, value in sorted(state.items()) if key not in aliases}
        before = {key: tensor_identity(value) for key, value in tensors.items()}
        output = args.output / model['model_id']
        output.mkdir()
        converted = output / 'model.safetensors'
        save_file(tensors, converted)
        restored = load_file(converted, device='cpu')
        after = {key: tensor_identity(value) for key, value in restored.items()}
        if before != after:
            raise ValueError('转换后张量与原件的dtype/shape/bitwise值不一致')
        # 第二次序列化验证本工具链输出内容可复现，不推广为跨版本等价。
        second = output / 'repeated.safetensors'
        save_file(tensors, second)
        if sha_file(second) != sha_file(converted):
            raise ValueError('同工具链重复转换哈希不一致')
        second.unlink()
        report = {'model_id': model['model_id'], 'revision': model['revision'], 'original_sha256': original['sha256'],
                  'file': 'model.safetensors', 'bytes': converted.stat().st_size, 'sha256': sha_file(converted),
                  'tensor_count': len(before), 'tensors_bitwise_verified': True, 'tensors': before,
                  'removed_aliases': aliases, 'repeated_serialization_identical': True}
        (output / 'conversion.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
        reports.append({key: value for key, value in report.items() if key not in ['tensors', 'removed_aliases']})
        del state, tensors, restored
    result = {'schema': 1, 'models': reports, 'tool_source_sha256': sha_file(Path(__file__)),
              'tools': {name: importlib.metadata.version(name) for name in ['torch', 'safetensors', 'numpy', 'packaging']},
              'scope': 'conversion bitwise verification; no formal translation quality or redistribution approval'}
    (args.output / 'converted-lock.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps(result, ensure_ascii=False))


if __name__ == '__main__':
    main()
