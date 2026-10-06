using System;
using System.Runtime.InteropServices;

public class pHash
{
	static pHash() {
		Environment.SetEnvironmentVariable("PATH", Environment.GetEnvironmentVariable("PATH")+
							":/usr/local/lib");
	}

	[DllImport("pHash", CharSet=CharSet.Ansi)]
	public static extern int ph_dct_imagehash(string file, ref ulong hash);

	[DllImport("pHash", CharSet=CharSet.Ansi)]
	public static extern IntPtr ph_mh_imagehash(string file, ref int n, float alpha,
						float lvl);

	// Marr-Hildreth image hash from raw interleaved pixel data held in
	// memory. pixels: row-major interleaved bytes (channels=1 -> luminance,
	// channels=3 -> RGB, channels=4 -> RGBA with alpha discarded). Length
	// must be width*height*channels bytes; the call returns IntPtr.Zero
	// on too-short buffers or bad arguments. On success, returns an IntPtr
	// to a malloc'd 72-byte hash that the caller must free() via pHash.free.
	[DllImport("pHash", CharSet=CharSet.Ansi)]
	public static extern IntPtr ph_mh_imagehash_from_pixels(byte[] pixels,
						int width, int height, int channels,
						ref int n, float alpha, float lvl);

	[DllImport("pHash", CharSet=CharSet.Ansi)]
	public static extern IntPtr ph_audiohash(IntPtr buf, int nbbuf, UInt32[] hashbuf,
						int nbcap, int sr, ref int nbframes);

	[DllImport("pHash", CharSet=CharSet.Ansi)]
	private static extern IntPtr ph_readaudio(string file, int sr, int ch,
					float[] buf, ref int buflen,
					float nbsecs);
	[DllImport("libc")]
	public static extern void free(IntPtr p);

}
