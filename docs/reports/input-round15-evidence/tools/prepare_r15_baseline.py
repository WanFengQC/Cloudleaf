from pathlib import Path
import io
import subprocess
import tarfile
root = Path(__file__).resolve().parents[1]
destination = root / '.local/round15-baseline'
destination.mkdir(exist_ok=False)
data = subprocess.check_output(['git', 'archive', '--format=tar', '75e0ace', 'Cargo.toml', 'Cargo.lock',
    'rust-toolchain.toml', 'crates', 'apps', 'adapters'], cwd=root)
with tarfile.open(fileobj=io.BytesIO(data)) as archive:
    archive.extractall(destination, filter='data')
print('Baseline is an unmodified archive of 75e0ace; no branch or worktree changed.')
