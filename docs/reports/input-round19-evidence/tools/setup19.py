from pathlib import Path
root=Path(__file__).resolve().parents[1]
for name in ['remote','discover','collect_ci','download_ci','launch']:
    s=(root/'.local'/f'{name}18.py').read_text(encoding='utf-8').replace('round18','round19').replace('Round18','Round19').replace('Round 18','Round 19').replace('R18','R19')
    (root/'.local'/f'{name}19.py').write_text(s,encoding='utf-8')
s=(root/'.local/full18_verified.py').read_text(encoding='utf-8').replace('round18','round19').replace('run18.py','run19.py')
(root/'.local/full19.py').write_text(s,encoding='utf-8')
p=root/'.local/launch19.py';s=p.read_text(encoding='utf-8').replace('import hashlib,json,subprocess,sys','import hashlib,json,subprocess,sys,os')
s=s.replace('p=subprocess.Popen(args,','env=os.environ.copy();env[\'CLOUDLEAF_USER_TEST_PROFILE\']=\'Round19-A\'\n    p=subprocess.Popen(args,env=env,')
s=s.replace("'only_owned_window':True","'user_profile':'Round19-A','only_owned_window':True")
p.write_text(s,encoding='utf-8')
