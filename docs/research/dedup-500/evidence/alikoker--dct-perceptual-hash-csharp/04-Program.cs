// SPDX-License-Identifier: Apache-2.0

using System;
using System.Drawing;
using MakLib.Imaging;

namespace DctPerceptualHash.Tests
{
    internal static class Program
    {
        private static int Main()
        {
            try
            {
                SupportedExtensions();
                DeterministicHash();
                SmallImageFailsFast();
                Console.WriteLine("All tests passed.");
                return 0;
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine(ex.Message);
                return 1;
            }
        }

        private static void SupportedExtensions()
        {
            Assert(PerceptualHash.IsSupportedFileExtension("image.bmp"), "BMP must be supported.");
            Assert(PerceptualHash.IsSupportedFileExtension("image.JPEG"), "JPEG matching must be case-insensitive.");
            Assert(PerceptualHash.IsSupportedFileExtension("image.png"), "PNG must be supported.");
            Assert(PerceptualHash.IsSupportedFileExtension("image.tiff"), "TIFF must be supported.");
            Assert(!PerceptualHash.IsSupportedFileExtension("image.webp"), "WebP must be rejected by the compatibility file filter.");
        }

        private static void DeterministicHash()
        {
            using (Bitmap bitmap = CreateReferenceBitmap())
            {
                ulong first = PerceptualHash.Compute(bitmap);
                ulong second = PerceptualHash.Compute(bitmap);
                Assert(first == second, "The same bitmap must produce the same 64-bit hash.");
            }
        }

        private static void SmallImageFailsFast()
        {
            using (Bitmap bitmap = new Bitmap(32, 32))
            {
                try
                {
                    PerceptualHash.Compute(bitmap);
                }
                catch (ArgumentException)
                {
                    return;
                }
            }

            throw new InvalidOperationException("Images smaller than 64x64 must fail explicitly.");
        }

        private static Bitmap CreateReferenceBitmap()
        {
            Bitmap bitmap = new Bitmap(96, 96);
            for (int y = 0; y < bitmap.Height; y++)
            {
                for (int x = 0; x < bitmap.Width; x++)
                {
                    int r = (x * 255) / (bitmap.Width - 1);
                    int g = (y * 255) / (bitmap.Height - 1);
                    int b = ((x + y) * 255) / (bitmap.Width + bitmap.Height - 2);
                    bitmap.SetPixel(x, y, Color.FromArgb(r, g, b));
                }
            }

            return bitmap;
        }

        private static void Assert(bool condition, string message)
        {
            if (!condition)
            {
                throw new InvalidOperationException(message);
            }
        }
    }
}
