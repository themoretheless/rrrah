"""Original complete parser and utility modules, standalone package binding only."""
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import types

root = Path(sys.argv[1]).resolve()
package = types.ModuleType('original_midi_control')
package.__path__ = []
sys.modules[package.__name__] = package
hashes = {}
for name, file in [('Util', '14-Util.py'), ('MidiParser', '12-MidiParser.py')]:
    path = root / file
    hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    spec = importlib.util.spec_from_file_location(f'{package.__name__}.{name}', path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    setattr(package, name, module)
Parser = package.MidiParser.MidiParser
with tempfile.TemporaryDirectory(prefix='midi-parser-control-') as tmp:
    path = Path(tmp) / 'fixture.mid'
    header = b'MThd' + (6).to_bytes(4, 'big')
    body = b'\x00\x00\x00\x01\x00\x60'
    track = b'MTrk' + (4).to_bytes(4, 'big')
    end_event = b'\x00\xff\x2f\x00'
    path.write_bytes(header + body + track + end_event)
    parser = Parser(path)
    try:
        pieces = []
        while parser.has_more_data():
            pieces.append(parser.read_next_data())
        assert pieces == [header, body, track, end_event]
    finally:
        parser.close()
    path.write_bytes(header + body[:2])
    parser = Parser(path)
    try:
        assert parser.read_next_data() == header
        returned = parser.read_next_data()
        assert returned == body[:2]
        assert not parser.has_more_data()
    finally:
        parser.close()
    path.write_bytes(header + body + b'MTrk' + (1).to_bytes(4, 'big') + b'\x80')
    parser = Parser(path)
    try:
        assert parser.read_next_data() == header
        assert parser.read_next_data() == body
        assert parser.read_next_data() == b'MTrk\x00\x00\x00\x01'
        try:
            parser.read_next_data()
        except IndexError:
            malformed_result = 'IndexError'
        else:
            raise AssertionError('expected original EOF IndexError was not observed')
    finally:
        parser.close()
print(json.dumps({'source_sha256': hashes, 'controls': 3, 'passed': 3,
                  'valid_file_pieces': len(pieces),
                  'declared_header_bytes': 6, 'truncated_header_returned_bytes': len(returned),
                  'unterminated_variable_length_result': malformed_result,
                  'scope': 'Whole original Python parser/utility modules with import package binding only, no rewritten code or Blender. All files owned temporary fixtures. Counterexample assertions, not fixes or full MIDI qualification.'}, indent=2))
