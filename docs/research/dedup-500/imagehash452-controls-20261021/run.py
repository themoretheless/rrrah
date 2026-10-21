"""Independent README/representation arithmetic, NOT original PHP execution."""
pairs=[('8f9e9d8b0f0f1f07','8e9e958b0f2f1f07',3),('6c2b58432011e38e','8f9e9d8b0f0f1f07',35)]
for a,b,want in pairs:
 actual=(int(a,16)^int(b,16)).bit_count();assert actual==want
 print('PASS README Hamming example:',actual)
def pack_chunks(bits):return bytes(int(bits[i:i+8],2) for i in range(0,len(bits),8))
assert pack_chunks('000000001')==pack_chunks('0000000000000001')==bytes([0,1])
print('PASS independent model: 9-bit and 16-bit inputs share packed bytes when last chunk left-padded')
print('3 independent arithmetic controls passed; no PHP, image driver, GMP or original suite executed')
