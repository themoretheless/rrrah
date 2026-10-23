from pathlib import Path
import sys,json
root=Path(__file__).resolve().parents[3];sys.path.insert(0,str(root/".dedup-work/mail092-20261024"))
from mail_deduplicate.cli import normalize_headers,mdedup
from click.testing import CliRunner
assert normalize_headers(None,None,("Subject","SUBJECT","From"))==("subject","from")
try:normalize_headers(None,None,("",))
except ValueError as error:assert "empty" in str(error)
else:raise AssertionError("empty header accepted")
result=CliRunner().invoke(mdedup,["--hash-header=","--hash-only"])
assert result.exit_code==1 and isinstance(result.exception,ValueError)
ordinary=CliRunner().invoke(mdedup,["--hash-header=contains space","--hash-only"])
assert ordinary.exit_code==2
out=dict(scope="Normally imported original callback and CLI; no mailbox arguments or actions",empty_header_cli_exit=result.exit_code,empty_header_exception=type(result.exception).__name__,ordinary_invalid_header_cli_exit=ordinary.exit_code,normal_header_dedup_lowercase=True,limits=["Input validation only, not mail grouping or recall proof."])
(root/"docs/research/dedup-500/mail092-empty-header-result-20261024.json").write_text(json.dumps(out,indent=2)+"\n");print(json.dumps(out))
