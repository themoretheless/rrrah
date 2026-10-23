"""Load all original MIDI-analysis modules; only package names are supplied."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import types

sys.dont_write_bytecode = True
root = Path(sys.argv[1]).resolve()
package = types.ModuleType('original_midi_analysis')
package.__path__ = []
sys.modules[package.__name__] = package
sources = {}
for name, file in [('Util','14-Util.py'), ('MidiParser','12-MidiParser.py'),
                   ('MidiEvents','18-MidiEvents.py'), ('MidiEventDecoder','13-MidiEventDecoder.py'),
                   ('Note','17-Note.py'), ('TrackData','16-TrackData.py'), ('MidiData','15-MidiData.py')]:
    path = root / file
    sources[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    spec = importlib.util.spec_from_file_location(f'{package.__name__}.{name}', path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    setattr(package, name, module)
Data = package.MidiData.MidiData
with tempfile.TemporaryDirectory(prefix='midi-analysis-control-') as tmp:
    path = Path(tmp) / 'fixture.mid'
    header = b'MThd' + (6).to_bytes(4,'big')
    body = b'\x00\x00\x00\x01\x00\x60'
    end = b'MTrk' + (4).to_bytes(4,'big') + b'\x00\xff\x2f\x00'
    def analyze(raw):
        path.write_bytes(raw)
        data = Data.__new__(Data)
        try:
            data.__init__(path)
            return {'format':data.format,'declared_tracks':data.num_tracks,
                    'parsed_tracks':data.get_num_tracks(),'ticks_per_beat':data.ticks_per_beat}
        finally:
            if hasattr(data,'event_decoder'):
                data.event_decoder.close()
    valid = analyze(header+body+end)
    assert valid == {'format':0,'declared_tracks':1,'parsed_tracks':1,'ticks_per_beat':96}
    short = analyze(header+body[:2])
    assert short == {'format':0,'declared_tracks':0,'parsed_tracks':0,'ticks_per_beat':0}
    missing = analyze(header+body)
    assert missing == {'format':0,'declared_tracks':1,'parsed_tracks':0,'ticks_per_beat':96}
    try:
        analyze(header+body+b'MTrk'+(1).to_bytes(4,'big')+b'\x80')
    except IndexError:
        unterminated = 'IndexError'
    else:
        raise AssertionError('original unterminated VLQ failure not observed')
print(json.dumps({'source_sha256':sources,'controls':4,'passed':4,
                  'valid':valid,'short_header_accepted':short,
                  'missing_declared_track_accepted':missing,
                  'unterminated_variable_length':unterminated,
                  'scope':'All seven original complete MIDI-analysis module bodies, no stubs or rewritten parsing. Bounded owned temporary inputs. Harness closes decoder in finally; does not qualify original resource cleanup, Blender/UI or all MIDI behavior.'},indent=2))
