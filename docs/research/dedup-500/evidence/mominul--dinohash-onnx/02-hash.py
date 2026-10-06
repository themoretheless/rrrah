from pathlib import Path
from PIL import Image
from typing import Optional, Union
import os

from .downloader import download_model
from .model import load_model
from .inference import hash

MODEL_URL="https://huggingface.co/backslashh/DINOHash/resolve/main/dinov2_vitb14_reg_512bit_dynamic.onnx"
MODEL_FILENAME="dinov2_vitb14_reg_512bit.onnx"

class DINOHash:
    def __init__(self, cache_dir: Optional[Path] = None):
        if cache_dir is None:
            cache_dir = Path.home() / ".cache" / "dinohash_onnx"
        self.cache_dir = Path(cache_dir)
        self.cache_dir.mkdir(parents=True, exist_ok=True)

        model_path = os.path.join(self.cache_dir, MODEL_FILENAME)

        download_model(MODEL_URL, model_path)

        self.session = load_model(model_path, device='cpu')

    def hash(self, images: Union[Image.Image, str]) -> str:
        if isinstance(images, Image.Image):
            images = [images]
        elif isinstance(images, str):
            images = [Image.open(images)]

        results = hash(self.session, images)

        [hash_result] = results
        hash_string = ''.join('1' if bit else '0' for bit in hash_result)
        hash_hex = hex(int(hash_string, 2))

        return hash_hex
