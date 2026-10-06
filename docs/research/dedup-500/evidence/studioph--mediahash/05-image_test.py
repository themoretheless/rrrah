import pathlib
from os import PathLike

import pytest

from mediahash import image
from mediahash.types import MediaImpl

TEST_DATA_FOLDER = pathlib.Path(__file__).parent / "data"


@pytest.mark.parametrize(
    ("filename", "expected"),
    [
        (
            TEST_DATA_FOLDER / "frame.jpg",
            image.ImageInfo(size=93722, width=1280, height=720, format="JPEG"),
        )
    ],
)
def test_image_info(filename: PathLike, expected):
    with open(filename, "rb") as fileobj:
        info = image.analyze(fileobj)
    assert info == expected
