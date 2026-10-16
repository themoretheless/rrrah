"""Exercise complete unchanged helper module; no cppyy/native hashing or package-init adapter."""
from pathlib import Path
import importlib.util,json,sys,hashlib
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/phash434-20261016';source=W/'source/pHashPy/helpers.py';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
spec=importlib.util.spec_from_file_location('owned_phash434_helpers',source);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
def run():
 mismatches=[];hist={};correct=0
 for a in range(256):
  for b in range(256):
   observed=module.hamming_bytes(bytes([a]),bytes([b]));oracle=sum(((a^b)>>i)&1 for i in range(8))/8
   if observed!=oracle:
    if len(mismatches)<16:mismatches.append({'a':a,'b':b,'observed':observed,'fixed_width_distance':oracle})
    hist[str((a^b).bit_length())]=hist.get(str((a^b).bit_length()),0)+1
   else:correct+=1
 assert correct==33024 and sum(hist.values())==32512
 controls=[('identity',b'\x12',b'\x12',0.0),('low_bit',b'\x00',b'\x01',1.0),('high_bit',b'\x00',b'\x80',0.125),('all_bits',b'\x00',b'\xff',1.0),('empty_arrays',b'',b'',0.0),('unequal_length_zero',b'',b'\0',0.0),('two_byte_low_bit',b'\0\0',(1).to_bytes(2,sys.byteorder),1.0)]
 rows=[]
 for name,a,b,expected in controls:
  got=module.hamming_bytes(a,b);assert got==expected;rows.append({'name':name,'a_hex':a.hex(),'b_hex':b.hex(),'observed':got,'expected':expected})
 bit_flips=[]
 for bit in range(64):
  observed=module.hamming_bytes(bytes(8),(1<<bit).to_bytes(8,sys.byteorder));assert observed==1/(bit+1)
  bit_flips.append({'bit':bit,'observed':observed,'fixed_width_distance':1/64})
 return {'one_byte_pairs':65536,'fixed_width_mismatches':32512,'fixed_width_correct':33024,'mismatch_bit_length_histogram':hist,'first_mismatches':mismatches,'controls':rows,'eight_byte_single_bit_positions':bit_flips}
runs=[run(),run()];assert runs[0]==runs[1]
p=R/'docs/research/dedup-500/phash434-hamming-controls-20261016.json';assert not p.exists();p.write_text(json.dumps({'status':'verified_native_helper_contract_counterexamples','producer_sha256':h(Path(__file__)),'source':str(source.relative_to(R)),'source_sha256':h(source),'runtime':sys.version,'byteorder':sys.byteorder,'runs':runs,'scope':'Completeunchangedhelpers.py loadedviaimportlib exactfilepath, actualstdlibint/bin. No functionextraction/dependencyfakes/C++calls/packageinitializer/initexecution.131072singlebytepaircomparisons plus128bitpositionchecks and14explicitboundarychecks across2rounds inoneprocess.','findings':['Helperdividespopcountxor bysignificantbitlength, notdeclaredhashwidth: one-bitdistancevaries1..1/64 bybitposition.32512of65536onebytepairs differfromnormalizedfixed-widthHammingdistance.','Functiondocdescribesmatchfractionbutidenticalhashreturns0, alloppositebitsreturns1: actualnormalizeddistance-like behaviornotmatchsimilarity.','Emptyandunequal-lengthzerobytehashes acceptedwith0, no equal-length/nonemptyadmission.','No timing/quality/runtimebinding orwholepHashPy/nativepHashqualification.']},indent=2)+'\n');print('Verified131072bytepairs,65024fixed-widthmismatches,128bitpositions,14boundaries;helperonly')
