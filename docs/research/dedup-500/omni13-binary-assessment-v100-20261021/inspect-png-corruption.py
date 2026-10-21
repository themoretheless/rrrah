from pathlib import Path
import zlib,struct,json,hashlib
C=Path('.dedup-work/omni13-20261021');original=(C/'android--app--src--test--screenshots--greeting.png').read_bytes();r=original.decode('utf8').encode('latin1');assert r[:7]==b'\x89PNG\n\x1a\n';off=32;length=struct.unpack_from('>I',r,off)[0];body=bytearray(r[off+4:off+8+length]);crcbytes=bytearray(r[off+8+length:off+12+length]);positions=[i for i,v in enumerate(body) if v==10];crc_positions=[i for i,v in enumerate(crcbytes) if v==10]
base=zlib.crc32(body)^int.from_bytes(crcbytes,'big');columns=[]
for i in positions:
 trial=body.copy();trial[i]=13;columns.append(zlib.crc32(trial)^zlib.crc32(body))
for i in crc_positions:
 trial=crcbytes.copy();trial[i]=13;columns.append(int.from_bytes(trial,'big')^int.from_bytes(crcbytes,'big'))
basis={}
for i,v in enumerate(columns):
 mask=1<<i
 while v:
  bit=v.bit_length()-1
  if bit in basis:v^=basis[bit][0];mask^=basis[bit][1]
  else:basis[bit]=(v,mask);break
v=base;solution=0
while v:
 bit=v.bit_length()-1;assert bit in basis,'hypothesis inconsistent';v^=basis[bit][0];solution^=basis[bit][1]
assert len(basis)==len(columns),'hypothesis not uniquely determined'
flips=[]
for j,i in enumerate(positions):
 if solution>>j&1:body[i]=13;flips.append(off+4+i)
for j,i in enumerate(crc_positions,len(positions)):
 if solution>>j&1:crcbytes[i]=13;flips.append(off+8+length+i)
assert zlib.crc32(body)==int.from_bytes(crcbytes,'big');inflated=zlib.decompress(body[4:]);width,height,depth,color,compression,filtering,interlace=struct.unpack('>IIBBBBB',r[15:28]);assert (depth,color,interlace)==(8,0,0);assert len(inflated)==height*(width+1);assert all(inflated[i*(width+1)] in range(5) for i in range(height))
# This is a candidate under an explicit corruption hypothesis, never a replacement of original bytes.
fixed=bytearray(r);fixed[off+4:off+8+length]=body;fixed[off+8+length:off+12+length]=crcbytes;fixed[7:11]=struct.pack('>I',13);candidate=b'\x89PNG\r\n\x1a\n'+bytes(fixed[7:]);(C/'greeting-diagnostic-candidate-v100.png').write_bytes(candidate)
report={'original_sha256':hashlib.sha256(original).hexdigest(),'original_bytes':len(original),'strict_utf8_codepoints':len(r),'all_codepoints_latin1_representable':True,'candidate_bytes':len(candidate),'candidate_sha256':hashlib.sha256(candidate).hexdigest(),'hypothesis':'Latin1 byte values re-encoded as UTF8 plus universal-newline normalization CR/CRLF to LF. Original producer and intended pixels not proven.','hypothesis_dependent_changes':{'signature':'reinsert CR before first LF','IHDR_length':'10 to 13','IDAT_or_CRC_LF_to_CR_offsets_in_singlebyte_stream':flips},'crc_linear_system':{'unknowns':len(columns),'rank':len(basis),'unique_under_hypothesis':True},'zlib_decompression':{'bytes':len(inflated),'expected_filtered_bytes':height*(width+1),'scanline_filter_bytes_valid':True},'width':width,'height':height,'bit_depth':depth,'color_type':color,'limits':'Original malformed PNG remains unchanged. Candidate validates only an explicit reversible-damage hypothesis, not original source provenance or expected screenshot correctness.'};(C/'png-forensics-v100.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
