import pytest
import hashlib
from pathlib import Path
from hscommon import desktop
@pytest.fixture(autouse=True)
def isolated_appdata(monkeypatch, request):
    path=Path(__file__).parent/'private-appdata'/hashlib.sha256(request.node.nodeid.encode()).hexdigest()
    path.mkdir(parents=True)
    monkeypatch.setattr(desktop, '_special_folder_path', lambda *args, **kwargs: str(path))
