from pathlib import Path
root = Path(__file__).resolve().parents[1]
cases = [
    ('polyphonic-bank', '<Esc> yinhang'), ('polyphonic-city', '<Esc> chongqing'),
    ('short-thanks', '<Esc> xiexie'), ('short-capital', '<Esc> beijing'),
    ('short-port', '<Esc> shanghai'), ('short-people', '<Esc> renmin'),
    ('compound-language', '<Esc> zhongwen'), ('compound-input', '<Esc> shurufa'),
    ('homophone-test', '<Esc> ceshi'), ('ambiguous-repeat', '<Esc> shishishishi'),
    ('long-people', '<Esc> zhonghuarenmingongheguo'), ('long-study', '<Esc> womenyqixuexizhongwen'),
    ('unknown-suffix', '<Esc> zhongwenqz'), ('unknown-reversed', '<Esc> nihoa'),
    ('middle-delete', '<Esc> nihao <Left> <Backspace>'),
    ('boundary-edit', '<Esc> xian <Left> <Left> <Left> <Left> <Right> <Right> \''),
    ('punctuation-preserved', '<Esc> nihao ,'), ('unknown-enter', '<Esc> qz <Enter>'),
]
payload = '# 原创研发输入，只用于行为与诊断，不是独立gold。\n# case\tevents\n' + ''.join(f'{name}\t{events}\n' for name, events in cases)
(root / '.local/round15-extra-events.tsv').write_text(payload, encoding='utf-8', newline='\n')
text = (root / '.local/trace15.py').read_text(encoding='utf-8')
text = text.replace("root/'tests/fixtures/input-round14-events.tsv'", "root/'.local/round15-extra-events.tsv'")
(root / '.local/trace15_extra.py').write_text(text, encoding='utf-8', newline='\n')
