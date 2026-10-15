package io.github.yuvraj0028.utils;

import java.awt.Graphics2D;
import java.awt.Image;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.IOException;

import javax.imageio.ImageIO;

/**
 * Utility class for basic image manipulation required for perceptual hashing, 
 * including reading, resizing, grayscale conversion, and pixel value extraction.
 */
public class ImageUtils {
    
    // Private constructor to prevent instantiation of this static utility class.
    private ImageUtils() {}

    /**
     * Reads an image from the specified file.
     * @param f The image file.
     * @return The image as a BufferedImage.
     * @throws IOException If the file cannot be read.
     */
    public static BufferedImage readImage(File f) throws IOException {
        return ImageIO.read(f);
    }

    /**
     * Converts a BufferedImage to a grayscale image.
     * @param img The source image.
     * @return A new BufferedImage in TYPE_BYTE_GRAY format.
     */
    public static BufferedImage toGrayscale(BufferedImage img) {
        // Create a new BufferedImage with the grayscale type.
        BufferedImage gray = new BufferedImage(img.getWidth(), img.getHeight(), BufferedImage.TYPE_BYTE_GRAY);
        Graphics2D g = gray.createGraphics();
        // Draw the original image onto the grayscale canvas, which performs the conversion.
        g.drawImage(img, 0, 0, null);
        g.dispose();
        return gray;
    }

    /**
     * Resizes a BufferedImage to the specified dimensions using a smooth scaling algorithm.
     * The resized image is also converted to grayscale (TYPE_BYTE_GRAY).
     * @param img The source image.
     * @param width The target width.
     * @param height The target height.
     * @return The resized and grayscale BufferedImage.
     */
    public static BufferedImage resize(BufferedImage img, int width, int height) {
        // Step 1: Get a scaled instance using a high-quality (smooth) scaling algorithm.
        Image tmp = img.getScaledInstance(width, height, Image.SCALE_SMOOTH);
        // Step 2: Create a new BufferedImage to hold the result (must be grayscale for hashing).
        BufferedImage resized = new BufferedImage(width, height, BufferedImage.TYPE_BYTE_GRAY);
        
        Graphics2D g2d = resized.createGraphics();
        // Step 3: Draw the scaled instance onto the new BufferedImage.
        g2d.drawImage(tmp, 0, 0, null);
        g2d.dispose();
        return resized;
    }

    /**
     * Extracts the grayscale intensity value (0-255) of a pixel at (x, y).
     * This method assumes the image is already in a grayscale format (e.g., TYPE_BYTE_GRAY) 
     * where the RGB value holds the gray intensity in its least significant byte.
     * @param img The BufferedImage.
     * @param x The x-coordinate.
     * @param y The y-coordinate.
     * @return The grayscale intensity as an integer (0-255).
     */
    public static int getGray(BufferedImage img, int x, int y) {
        // Get the full 32-bit ARGB integer and mask it with 0xff to isolate the 
        // least significant byte (which holds the gray intensity in TYPE_BYTE_GRAY).
        return img.getRGB(x, y) & 0xff;
    }
}
