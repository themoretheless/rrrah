// SPDX-License-Identifier: Apache-2.0

using System;
using System.Globalization;
using System.IO;
using MakLib.Imaging;

namespace PerceptualHash.Cli
{
    internal static class Program
    {
        private const int Success = 0;
        private const int ProcessingError = 1;
        private const int UsageError = 2;

        private static int Main(string[] args)
        {
            if (args.Length == 0)
            {
                PrintUsage(Console.Error);
                return UsageError;
            }

            if (args.Length == 1 && (args[0] == "--help" || args[0] == "-h"))
            {
                PrintUsage(Console.Out);
                return Success;
            }

            int exitCode = Success;

            for (int i = 0; i < args.Length; i++)
            {
                string path = args[i];

                if (!File.Exists(path))
                {
                    Console.Error.WriteLine("phash: {0}: file not found", path);
                    exitCode = ProcessingError;
                    continue;
                }

                if (!PerceptualHash.IsSupportedFileExtension(path))
                {
                    Console.Error.WriteLine("phash: {0}: unsupported file extension", path);
                    exitCode = ProcessingError;
                    continue;
                }

                try
                {
                    ulong hash = PerceptualHash.Compute(path);
                    Console.Out.WriteLine("{0}  {1}  {2}",
                        hash.ToString("X16", CultureInfo.InvariantCulture),
                        hash.ToString(CultureInfo.InvariantCulture),
                        path);
                }
                catch (Exception ex)
                {
                    Console.Error.WriteLine("phash: {0}: {1}", path, ex.Message);
                    exitCode = ProcessingError;
                }
            }

            return exitCode;
        }

        private static void PrintUsage(TextWriter writer)
        {
            writer.WriteLine("Usage: phash <image> [image ...]");
            writer.WriteLine();
            writer.WriteLine("Computes a 64-bit DCT perceptual hash for each image.");
            writer.WriteLine("Output: <HEX64>  <DECIMAL64>  <PATH>");
            writer.WriteLine("Supported extensions: .bmp .gif .jpg .jpeg .png .tif .tiff");
        }
    }
}
