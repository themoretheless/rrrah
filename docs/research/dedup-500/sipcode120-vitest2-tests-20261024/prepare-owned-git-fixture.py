from pathlib import Path
import subprocess
root = Path(__file__).resolve().parent
if not (root / '.git').exists():
    subprocess.run(['git', 'init', '-q', str(root)], check=True)
    (root / 'owned-git-fixture.ts').write_text('export const ownedFixture = true;\n')
    subprocess.run(['git', '-C', str(root), 'add', '--', 'owned-git-fixture.ts'], check=True)
    subprocess.run(['git', '-C', str(root), '-c', 'user.name=Owned Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '-qm', 'owned upstream Git test fixture'], check=True)
