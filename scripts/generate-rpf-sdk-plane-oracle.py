#!/usr/bin/env python3
"""Generate byte-plane oracle records using an externally supplied Autodesk SDK.
SDK source stays outside the repository; only generated streams are retained.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('sdk_source', type=pathlib.Path)
parser.add_argument('output', type=pathlib.Path)
args = parser.parse_args()
source_bytes = args.sdk_source.read_bytes()
source = source_bytes.decode().replace('\r\n', '\n')
start = source.rindex('static int encode(unsigned char * input, unsigned char* output,')
brace = source.index('{', start)
depth = 1
end = brace + 1
while depth:
    depth += (source[end] == '{') - (source[end] == '}')
    end += 1
codec = source[start:end]
wrapper = r'''
#include <cstdio>
#include <vector>
static void hex(const std::vector<unsigned char>& b, int n) {
    for (int i=0; i<n; i++) std::printf("%02x", b[i]);
}
int main() {
    int widths[] = {1,127,128,129,255,256,513};
    int sizes[] = {4,1,2,8,4,4,1,3,2,3,3,8,3,2};
    for (int channel=0; channel<14; channel++) for (int width: widths) {
        int size=sizes[channel];
        // Extra padding makes the SDK encoder's lookahead safe at literal boundaries.
        std::vector<unsigned char> input((width+1)*size, 0);
        for (int x=0; x<width; x++) for (int plane=0; plane<size; plane++)
            input[x*size+plane] = x<130 ? (plane*17+channel) : ((x*37+plane*13+channel)&255);
        std::vector<unsigned char> records;
        for (int plane=size-1; plane>=0; plane--) {
            std::vector<unsigned char> out(width*2+32, 0);
            int n=encode(input.data()+plane, out.data(), width, size);
            records.push_back((n>>8)&255); records.push_back(n&255);
            records.insert(records.end(), out.begin(), out.begin()+n);
        }
        std::printf("{\"channel\":%d,\"width\":%d,\"sample_bytes\":%d,\"expected_hex\":\"",channel,width,size);
        hex(input,width*size);
        std::printf("\",\"encoded_hex\":\""); hex(records,records.size()); std::printf("\"}\n");
    }
}
'''
with tempfile.TemporaryDirectory(prefix='rrrah-rpf-sdk-oracle-') as tmp:
    root = pathlib.Path(tmp)
    cpp = root / 'oracle.cpp'
    cpp.write_text('#include <cstdio>\n#include <vector>\n' + codec + '\n' + wrapper)
    exe = root / 'oracle'
    subprocess.run(['clang++', '-std=c++17', '-O2', str(cpp), '-o', str(exe)], check=True)
    records = [json.loads(line) for line in subprocess.check_output([str(exe)], text=True).splitlines()]
manifest = {
    'scope': 'Synthetic samples encoded by unmodified official SDK encode(); not a 3ds Max rendered file',
    'sdk_source_sha256': hashlib.sha256(source_bytes).hexdigest(),
    'encoder_source_sha256': hashlib.sha256(codec.encode()).hexdigest(),
    'cases': records,
}
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(manifest, indent=2) + '\n')
print(f'{len(records)} SDK encoder cases; {args.output.stat().st_size} bytes')
