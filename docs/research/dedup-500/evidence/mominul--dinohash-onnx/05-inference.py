"""Module for running inference and generating perceptual hashes."""

import numpy as np
from PIL import Image
from typing import List, Union
import onnxruntime as ort


def preprocess_images(images: List[Union[str, Image.Image]]) -> np.ndarray:
    """
    Preprocesses images for model input.

    Args:
        images: List of image paths (strings) or PIL Image objects

    Returns:
        Preprocessed tensor as numpy array with shape [batch_size, 3, 224, 224]
    """
    # ImageNet normalization parameters
    mean = np.array([0.485, 0.456, 0.406], dtype=np.float32)
    std = np.array([0.229, 0.224, 0.225], dtype=np.float32)

    batch_size = len(images)
    pixel_data = np.zeros((batch_size, 3, 224, 224), dtype=np.float32)

    for img_index, image in enumerate(images):
        # Load image if it's a path
        if isinstance(image, str):
            img = Image.open(image)
        else:
            img = image

        # Convert to RGB if needed
        if img.mode != 'RGB':
            img = img.convert('RGB')

        # Resize to 224x224
        img = img.resize((224, 224), Image.BILINEAR)

        # Convert to numpy array and normalize to [0, 1]
        img_array = np.array(img, dtype=np.float32) / 255.0

        # Apply channel-wise normalization and transpose to CHW format
        for c in range(3):
            pixel_data[img_index, c, :, :] = (img_array[:, :, c] - mean[c]) / std[c]

    return pixel_data


def hash(session: ort.InferenceSession, images: List[Union[str, Image.Image]]) -> List[List[bool]]:
    """
    Generates perceptual hashes for images using a loaded ONNX model.

    Args:
        session: Loaded ONNX Runtime inference session
        images: List of image paths (strings) or PIL Image objects

    Returns:
        List of hashes, where each hash is a list of boolean values

    Raises:
        Exception: If inference fails
    """
    try:
        # Preprocess images
        tensor = preprocess_images(images)

        # Run inference
        input_name = session.get_inputs()[0].name
        output_name = session.get_outputs()[0].name

        results = session.run([output_name], {input_name: tensor})
        output_tensor = results[0]

        # Convert to boolean hashes
        bits = output_tensor.shape[1]
        flat_hash = (output_tensor >= 0).flatten().tolist()

        # Split into individual hashes
        hashes = []
        for i in range(0, len(flat_hash), bits):
            hashes.append(flat_hash[i:i + bits])

        return hashes

    except Exception as error:
        print(f"Error during inference: {error}")
        raise
