/** Base class for every error this library throws. */
export class BundleDupeCheckError extends Error {
  constructor(message: string) {
    super(message);
    this.name = new.target.name;
  }
}

/** A `--stats` file exists but isn't JSON, or isn't a shape we recognize. */
export class UnrecognizedStatsFormatError extends BundleDupeCheckError {
  constructor(path: string, reason: string) {
    super(`Could not read "${path}" as a stats file: ${reason}`);
  }
}

/** The path given to scan a build output directory doesn't exist or isn't a directory. */
export class InvalidDistPathError extends BundleDupeCheckError {
  constructor(path: string, reason: string) {
    super(`"${path}" is not usable as a build output directory: ${reason}`);
  }
}

/** Scanning a `dist/` directory found no source maps to attribute bytes with. */
export class NoSourceMapsFoundError extends BundleDupeCheckError {
  constructor(path: string) {
    super(
      `No source maps found under "${path}". Directory scanning needs sourcemaps to tell which ` +
        `package each byte came from — build with sourcemaps enabled, or capture a stats file ` +
        `(esbuild --metafile, rollup-plugin-visualizer, or this package's own plugin) and pass it with --stats.`,
    );
  }
}
