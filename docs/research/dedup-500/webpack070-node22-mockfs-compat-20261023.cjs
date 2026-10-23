// Test harness only: avoid Node22 native UTF8/exists fast paths bypassing mock-fs4.
// Does not modify pinned product or original test bodies.
const fs = require('fs');
const read = fs.readFileSync;
const write = fs.writeFileSync;
fs.existsSync = function (file) {
    try { fs.statSync(file); return true; } catch { return false; }
};
fs.readFileSync = function (file, options) {
    const encoding = typeof options === 'string' ? options : options && options.encoding;
    if (!encoding) return read.call(fs, file, options);
    const buffer = read.call(fs, file, typeof options === 'string' ? undefined : {...options, encoding:null});
    return buffer.toString(encoding);
};
fs.writeFileSync = function (file, data, options) {
    const encoding = typeof options === 'string' ? options : options && options.encoding;
    return write.call(fs, file, typeof data === 'string' ? Buffer.from(data, encoding || 'utf8') : data, options);
};
