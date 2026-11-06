from pathlib import Path
from html.parser import HTMLParser
import hashlib

root=Path(__file__).resolve().parents[1]
source=Path('C:/Users/EDY/.codex/tmp/r16-corpus/CC-BY-2.0-FR-legalcode.html')
class LegalCode(HTMLParser):
    def __init__(self):
        super().__init__(); self.depth=0; self.parts=[]
    def handle_starttag(self,tag,attrs):
        if tag=='div' and (self.depth or dict(attrs).get('id')=='legal-code-body'): self.depth+=1
    def handle_endtag(self,tag):
        if tag=='div' and self.depth: self.depth-=1
    def handle_data(self,data):
        if self.depth and data.strip(): self.parts.append(' '.join(data.split()))
parser=LegalCode();parser.feed(source.read_text(encoding='utf-8'))
content='\n'.join(parser.parts)+'\n'
assert len(content) > 10000 and 'Contrat' in content and 'Offrant' in content
destination=root/'licenses/input-context/CC-BY-2.0-FR.txt'
payload='CC BY 2.0 FR\nhttps://creativecommons.org/licenses/by/2.0/fr/legalcode\nSource HTML SHA-256: '+hashlib.sha256(source.read_bytes()).hexdigest()+'\nOnly HTML markup and whitespace formatting removed; license words unchanged.\n\n'+content
destination.write_text(payload,encoding='utf-8',newline='\n')
print({'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'license_sha256':hashlib.sha256(destination.read_bytes()).hexdigest(),'bytes':destination.stat().st_size})
original=(root/'.local/full16.py').read_text(encoding='utf-8')
original=original.replace("label=f'final-{i:02}'", "label=f'verified-{i:02}'").replace('round16-full-results.json','round16-verified-results.json')
(root/'.local/full16_verified.py').write_text(original,encoding='utf-8',newline='\n')
