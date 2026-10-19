"""Inspect original MusicFile metadata using a valid short task-owned AIFF fixture."""
from pathlib import Path
import aifc
import hashlib
import json
import math
import struct
import subprocess
import sys

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/dupeguru1-20261019/source'
W=R/'.dedup-work/dupeguru1-aiff-metadata-20261019'
H=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    W.mkdir(exist_ok=False)
    source='core/me/fs.py';pin=H(S/source)
    assert subprocess.check_output(['git','-C',str(S),'show','HEAD:'+source])==(S/source).read_bytes()
    path=W/'tone.aiff';rate=16000;count=1600
    frames=b''.join(struct.pack('>h',int(4000*math.sin(2*math.pi*440*i/rate)))for i in range(count))
    with aifc.open(str(path),'wb')as stream:
        stream.setnchannels(1);stream.setsampwidth(2);stream.setframerate(rate);stream.writeframes(frames)
    from mutagen.aiff import AIFF
    from mutagen.id3 import TPE1,TIT2
    tagged=AIFF(path);tagged.add_tags();tagged.tags.add(TPE1(encoding=3,text=['Fixture Artist']));tagged.tags.add(TIT2(encoding=3,text=['Fixture Title']));tagged.save()
    fresh=AIFF(path);assert fresh.tags['TPE1'].text==['Fixture Artist']and fresh.tags['TIT2'].text==['Fixture Title']
    assert fresh.info.sample_rate==rate and fresh.info.channels==1
    sys.path.insert(0,str(S))
    from core.me.fs import MusicFile
    supported=MusicFile.can_handle(path);assert supported
    wrapped=MusicFile(path)
    actual={field:getattr(wrapped,field)for field in ['artist','title','duration','samplerate','bitrate']}
    assert H(S/source)==pin
    result={'status':'executed_original_MusicFile_on_valid_tagged_aiff','source_pin':{source:pin},'fixture_sha256':H(path),'native_mutagen_tags':{'artist':fresh.tags['TPE1'].text,'title':fresh.tags['TIT2'].text},'native_mutagen_info':{'sample_rate':fresh.info.sample_rate,'channels':fresh.info.channels,'length':fresh.info.length},'MusicFile_can_handle':supported,'MusicFile_fields':actual,'tag_values_preserved':actual['artist']=='Fixture Artist'and actual['title']=='Fixture Title','limits':['Valid 0.1second synthetic AIFF with real PCM and real Mutagen ID3 serialization; no mocked tag or audio provider.','Single format/version control, not every music container/codec or auditory-identity qualification.','No audio playback, user data, deletion action or production source change.']}
    (W/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))

if __name__=='__main__':main()
