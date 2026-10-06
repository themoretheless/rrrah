"""Module for downloading ONNX models from remote URLs."""

import os
import requests
from pathlib import Path
from tqdm import tqdm


def download_model(url: str, output_path: str) -> str:
    """
    Downloads a model from a URL with progress bar.

    Args:
        url: URL to download the model from
        output_path: Path to save the model to

    Returns:
        Path to the downloaded model

    Raises:
        Exception: If download fails
    """
    try:
        # Create directory if it doesn't exist
        output_dir = os.path.dirname(output_path)
        if output_dir:
            Path(output_dir).mkdir(parents=True, exist_ok=True)

        # Check if file already exists
        if os.path.exists(output_path):
            print(f"Model already exists at {output_path}")
            return output_path

        print(f"Downloading model from {url}...")

        # Download with progress bar
        response = requests.get(url, stream=True)
        response.raise_for_status()

        total_size = int(response.headers.get('content-length', 0))

        with open(output_path, 'wb') as file, tqdm(
            desc="Downloading",
            total=total_size,
            unit='iB',
            unit_scale=True,
            unit_divisor=1024,
        ) as progress_bar:
            for chunk in response.iter_content(chunk_size=8192):
                size = file.write(chunk)
                progress_bar.update(size)

        print(f"Model downloaded to {output_path}")
        return output_path

    except Exception as error:
        print(f"Error downloading model: {error}")
        # Clean up partial download
        if os.path.exists(output_path):
            os.remove(output_path)
        raise
