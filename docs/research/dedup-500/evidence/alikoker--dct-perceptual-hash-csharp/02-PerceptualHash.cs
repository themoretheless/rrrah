// SPDX-License-Identifier: Apache-2.0
// Author: Muhammet Ali Köker

using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Linq;
using System.Numerics;
using System.Runtime.CompilerServices;

namespace MakLib.Imaging
{
    public class PerceptualHash
    {
        private const int WorkingSize = 64;
        private const int LowFrequencySize = 8;
        private const int HashBitCount = 64;
        private const ulong InitialMask = 1UL << (HashBitCount - 1);
        private const PixelFormat WorkingPixelFormat = PixelFormat.Format24bppRgb;
        private static readonly float Sqrt2DivSize = (float)Math.Sqrt(2.0 / WorkingSize);
        private static readonly float Sqrt2 = (float)(1.0 / Math.Sqrt(2.0));
        private static readonly List<Vector4>[] DctCoefficientsSimd = GenerateDctCoefficientsSimd();
        private static readonly ReadOnlyCollection<string> SupportedExtensions = Array.AsReadOnly(new[]
        {
            ".bmp", ".gif", ".jpg", ".jpeg", ".png", ".tif", ".tiff"
        });

        public static IReadOnlyList<string> SupportedFileExtensions
        {
            get { return SupportedExtensions; }
        }

        public static bool IsSupportedFileExtension(string path)
        {
            if (string.IsNullOrWhiteSpace(path))
            {
                return false;
            }

            string extension = Path.GetExtension(path);
            for (int i = 0; i < SupportedExtensions.Count; i++)
            {
                if (string.Equals(extension, SupportedExtensions[i], StringComparison.OrdinalIgnoreCase))
                {
                    return true;
                }
            }

            return false;
        }

        public static ulong Compute(string path)
        {
            if (string.IsNullOrWhiteSpace(path))
            {
                throw new ArgumentException("A source image path is required.", nameof(path));
            }

            if (!IsSupportedFileExtension(path))
            {
                throw new NotSupportedException("Unsupported image extension: " + Path.GetExtension(path));
            }

            using (Bitmap bitmap = new Bitmap(path))
            {
                return Compute(bitmap);
            }
        }

        public static ulong Compute(Bitmap bmp)
        {
            if (bmp == null)
            {
                throw new ArgumentNullException(nameof(bmp));
            }

            if (bmp.Width < WorkingSize || bmp.Height < WorkingSize)
            {
                throw new ArgumentException("The source bitmap must be at least 64x64 pixels because the compatibility calibration pass samples a 64x64 region of the original image.", nameof(bmp));
            }

            float[,] rows = new float[WorkingSize, WorkingSize];
            float[] sequence = new float[WorkingSize];
            float[,] matrix = new float[WorkingSize, WorkingSize];
            int x;
            int y;

            using (Bitmap dst = new Bitmap(WorkingSize, WorkingSize, WorkingPixelFormat))
            {
                using (Graphics g = Graphics.FromImage(dst))
                {
                    y = dst.Width * bmp.Width / bmp.Height;
                    x = (dst.Width - y) >> 1;
                    g.DrawImage(bmp, new Rectangle(x, 0, y, dst.Height), new Rectangle(0, 0, bmp.Width, bmp.Height), GraphicsUnit.Pixel);
                }

                int maxR = 0;
                int maxG = 0;
                int maxB = 0;
                int minR = 255;
                int minG = 255;
                int minB = 255;
                Color c;

                for (x = 0; x < WorkingSize; x++)
                {
                    for (y = 0; y < WorkingSize; y++)
                    {
                        c = bmp.GetPixel(x, y);

                        if (minR > c.R)
                        {
                            minR = c.R;
                        }
                        else if (maxR < c.R)
                        {
                            maxR = c.R;
                        }

                        if (minG > c.G)
                        {
                            minG = c.G;
                        }
                        else if (maxG < c.G)
                        {
                            maxG = c.G;
                        }

                        if (minB > c.B)
                        {
                            minB = c.B;
                        }
                        else if (maxB < c.B)
                        {
                            maxB = c.B;
                        }
                    }
                }

                for (y = 0; y < WorkingSize; y++)
                {
                    for (x = 0; x < WorkingSize; x++)
                    {
                        c = dst.GetPixel(x, y);
                        sequence[x] = ((((c.R - minR) * 255.0f) / maxR) + (((c.G - minG) * 255.0f) / maxG) + (((c.B - minB) * 255.0f) / maxB)) / 3.0f;
                    }

                    Dct1DSimd(sequence, rows, y, WorkingSize);
                }
            }

            for (x = 0; x < LowFrequencySize; x++)
            {
                for (y = 0; y < WorkingSize; y++)
                {
                    sequence[y] = rows[y, x];
                }

                Dct1DSimd(sequence, matrix, x, LowFrequencySize);
            }

            float[] result = new float[HashBitCount];
            for (y = 0; y < LowFrequencySize; y++)
            {
                for (x = 0; x < LowFrequencySize; x++)
                {
                    result[(y << 3) + x] = matrix[y, x];
                }
            }

            float median = result.OrderBy(value => value).Skip(31).Take(2).Average();
            ulong mask = InitialMask;
            ulong hash = 0UL;

            for (x = 0; x < HashBitCount; x++)
            {
                if (result[x] > median)
                {
                    hash |= mask;
                }

                mask >>= 1;
            }

            return hash;
        }

        public static void Save(string source, string target)
        {
            ulong hash = Compute(source);
            File.AppendAllText(target, string.Concat(hash, Environment.NewLine));
        }

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        private static void Dct1DSimd(float[] raw, float[,] coefficients, int ci, int length)
        {
            List<Vector4> result = new List<Vector4>();
            int x;
            int y = 0;

            while (y < WorkingSize)
            {
                result.Add(new Vector4(raw[y++], raw[y++], raw[y++], raw[y++]));
            }

            for (x = 0; x < length; x++)
            {
                for (y = 0; y < result.Count; y++)
                {
                    coefficients[ci, x] += Vector4.Dot(result[y], DctCoefficientsSimd[x][y]);
                }

                coefficients[ci, x] *= Sqrt2DivSize;
                if (x == 0)
                {
                    coefficients[ci, x] *= Sqrt2;
                }
            }
        }

        // Discrete cosine transform coefficients are precomputed once and reused by every hash operation.
        private static List<Vector4>[] GenerateDctCoefficientsSimd()
        {
            List<Vector4>[] results = new List<Vector4>[WorkingSize];
            float[] raw = new float[WorkingSize];

            for (int x = 0; x < WorkingSize; x++)
            {
                for (int y = 0; y < WorkingSize; y++)
                {
                    raw[y] = (float)Math.Cos(((2.0 * y) + 1.0) * x * Math.PI / (2.0 * WorkingSize));
                }

                List<Vector4> result = new List<Vector4>();
                int index = 0;
                while (index < WorkingSize)
                {
                    result.Add(new Vector4(raw[index++], raw[index++], raw[index++], raw[index++]));
                }

                results[x] = result;
            }

            return results;
        }
    }
}
