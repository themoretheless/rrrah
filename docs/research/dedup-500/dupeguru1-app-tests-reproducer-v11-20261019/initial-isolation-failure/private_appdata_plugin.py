import pytest
from hscommon import desktop
@pytest.fixture(autouse=True)
def isolated_appdata(monkeypatch, tmp_path):
    path=tmp_path/'private-appdata'
    path.mkdir()
    monkeypatch.setattr(desktop, '_special_folder_path', lambda *args, **kwargs: str(path))
