from pathlib import Path
import hashlib
import json

root=Path(__file__).resolve().parents[1]
source=Path('C:/Users/EDY/.codex/tmp/r16-corpus/word-contrast/word-covered-char-train.txt')
header=(root/'.local/round16-language/metadata.tsv').read_text(encoding='utf-8')
header=header.replace('1878767d1f0076011cac0be2c67afdfa0256a961030a80d91503a145d007590e', hashlib.sha256(source.read_bytes()).hexdigest()).replace('tatoeba-cmn-20261031\n','tatoeba-cmn-20261031-word-covered\n')
(root/'.local/round16-language/covered-metadata.tsv').write_text(header,encoding='utf-8',newline='\n')
p=root/'.local/trace16.py';s=p.read_text(encoding='utf-8')
s=s.replace("variant=='original'", "variant.startswith('original')")
s=s.replace("if variant=='lexical':", "if variant in ['lexical','originallexical']:")
s=s.replace("if variant=='character':", "if variant in ['character','covered','word']:")
s=s.replace("model=root/'.local/round16-language/character.tsv'", "model=root/('.local/round16-language/character.tsv' if variant=='character' else '.local/round16-language/covered-character.tsv' if variant=='covered' else '.local/round16-language/word.tsv')")
s=s.replace("hexdigest(),'character_bigram'", "hexdigest(),'bigram' if variant=='word' else 'character_bigram'")
s=s.replace("raw=(out/'stdout.tsv')", """kernel=ctypes.WinDLL('kernel32',use_last_error=True)
kernel.GetProcessTimes.argtypes=[wintypes.HANDLE]+[ctypes.POINTER(wintypes.FILETIME)]*4
clock=[wintypes.FILETIME() for _ in range(4)]
cpu_ok=kernel.GetProcessTimes(int(process._handle),*[ctypes.byref(value) for value in clock])
cpu_seconds=sum((value.dwHighDateTime<<32)|value.dwLowDateTime for value in clock[2:])/10_000_000 if cpu_ok else None
raw=(out/'stdout.tsv')""")
s=s.replace("'variant':variant,", "'variant':variant,'whole_process_cpu_seconds':cpu_seconds,")
p.write_text(s,encoding='utf-8',newline='\n')
print('prepared trace variants and covered model metadata')
