const fs = require('fs');
const vm = require('vm');
const assert = require('assert/strict');
const path = require('path');
const directory = path.resolve(__dirname, 'evidence/DragonOfMath--dupe-images');
const mapping = JSON.parse(fs.readFileSync('/private/tmp/rrrah-dragon324-local-20261015.json'));
const source = name => fs.readFileSync(path.join(directory,
    name === 'dupe-image-checker.js' ? '02-dupe-image-checker.js' : mapping.find(x => x.path === name).local), 'utf8');
let checks = 0;
function check(value) { assert.ok(value); checks++; }

function environment(records, difference, cached = null) {
    const context = vm.createContext({ console: { log() {}, warn() {}, error() {} }, Promise });
    const modules = new Map();
    const observations = { reads: [], diffs: [] };
    const descriptions = Object.fromEntries(records.map(x => [x.name, x]));
    class SyntheticFile {
        constructor(filename) { this.name = filename; this.path = filename; }
        size() { return descriptions[this.name].size || 1; }
    }
    const adapters = {
        jimp: {
            read(filename) {
                observations.reads.push(filename);
                const record = descriptions[filename];
                return Promise.resolve({ name: filename, bitmap: { width: 8, height: 8 }, hash: () => record.hash || '0'.repeat(64) });
            },
            diff(a, b, threshold) {
                observations.diffs.push([a.name, b.name, threshold]);
                return { percent: difference(a.name, b.name) };
            }
        },
        './utils/read-all': () => Object.fromEntries(records.map(x => [x.name, x.name])),
        './utils/File': SyntheticFile,
        './FilePromise': {
            join: (...parts) => parts.join('/'), existsSync: () => cached !== null,
            readSync: () => cached, create: () => Promise.resolve()
        }
    };
    function load(name) {
        if (modules.has(name)) return modules.get(name).exports;
        const module = { exports: {} };
        modules.set(name, module);
        const requireAdapter = request => {
            if (Object.prototype.hasOwnProperty.call(adapters, request)) return adapters[request];
            const aliases = {
                './utils/Array': 'utils/array.js', './utils/hex': 'utils/hex.js',
                './utils/formatting': 'utils/formatting.js', './utils/FileCache': 'utils/FileCache.js',
                './utils/Logger': 'utils/Logger.js'
            };
            if (!aliases[request]) throw new Error('Unapproved dependency: ' + request);
            return load(aliases[request]);
        };
        const wrapper = vm.runInContext('(function(require,module,exports){\n' + source(name) + '\n})', context, { filename: name });
        wrapper(requireAdapter, module, module.exports);
        return module.exports;
    }
    return { context, load, observations };
}

(async () => {
    const records = ['a', 'b', 'c'].map(name => ({ name }));
    const differences = (a, b) => a === b || a === 'b' || b === 'b' ? 0 : 0.5;
    const central = environment([records[1], records[0], records[2]], differences);
    const centralGroups = await central.load('dupe-image-checker.js')('/synthetic');
    check(centralGroups.length === 1 && centralGroups[0].length === 3);
    check(!central.observations.diffs.some(([a, b]) => [a, b].includes('a') && [a, b].includes('c')));
    const endpoint = environment(records, differences);
    const endpointGroups = await endpoint.load('dupe-image-checker.js')('/synthetic');
    check(endpointGroups.length === 1 && endpointGroups[0].length === 2);
    const exact = environment(records.slice(0, 2), () => 0.005);
    const exactGroups = await exact.load('dupe-image-checker.js')('/synthetic', { exact: true });
    check(exactGroups.length === 1 && exactGroups[0].length === 2);
    const boundary = environment(records.slice(0, 2), () => 0.01);
    check((await boundary.load('dupe-image-checker.js')('/synthetic', { tolerance: 0.01 })).length === 0);
    const cached = {
        a: { width: 8, height: 8, _size: 1, _hash: '0'.repeat(16) },
        b: { width: 8, height: 8, _size: 1, _hash: 'f'.repeat(16) }
    };
    const stale = environment(records.slice(0, 2), () => 0, cached);
    check((await stale.load('dupe-image-checker.js')('/synthetic')).length === 0);
    check(stale.observations.reads.length === 0);
    const arrays = environment([], () => 0);
    arrays.load('utils/array.js');
    check(await vm.runInContext('let count=0; [].forEachAsync(value=>{if(value===undefined)count++;}).then(()=>count)', arrays.context) === 1);
    const empty = environment([], () => 0);
    assert.throws(() => empty.load('dupe-image-checker.js')('/synthetic'), /name/); checks++;
    const singleton = environment(records.slice(0, 1), () => 0);
    const singletonResult = await singleton.load('dupe-image-checker.js')('/synthetic');
    check(singletonResult && singletonResult.name === 'TypeError');

    // Whole original directory reader with a controlled filesystem inventory.
    const module = { exports: {} };
    const fakeFs = { readdirSync: () => ['same.jpg', 'same.png'], statSync: () => ({ isDirectory: () => false }) };
    vm.runInNewContext('(function(require,module,exports){\n' + source('utils/read-all.js') + '\n})')(
        name => { assert.equal(name, 'fs'); return fakeFs; }, module, module.exports);
    const listing = module.exports({ dirname: '/synthetic', filter: /([^\/]+)\.(jpe?g|png|bmp)$/i });
    check(Object.keys(listing).length === 1 && listing.same === '/synthetic/same.png');
    const cacheEnvironment = environment([], () => 0);
    const Cache = cacheEnvironment.load('utils/FileCache.js');
    check(new Cache('/synthetic', 'cache').has('__proto__'));
    console.log(JSON.stringify({ checks, central_group_size: 3, endpoint_group_size: 2,
        exact_ignored: true, stale_cache_skips_all_reads: true, basename_collision_loses_file: true,
        scope: 'Original checker/array/hex/cache/logger/formatting/directory-reader source executed unchanged in isolated NodeVM. Explicit synthetic Jimp, File, filesystem inventory and FilePromise adapters; case aliases explicit. No actual Jimp/image decode/hash quality/package/deletion/rename/explorer/network/private media or production adoption qualification.' }));
})().catch(error => { console.error(error); process.exitCode = 1; });
