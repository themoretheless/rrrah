"""Original whole dHash module with actual Pillow and synthetic independent vectors."""
import pathlib, importlib.util, json, hashlib, sys
import PIL
from PIL import Image

R = pathlib.Path(__file__).resolve().parents[1]
S = pathlib.Path('/Users/themoretheless/.codex/tmp/rrrah-rayraegah-dhash-source-20261013')
spec = importlib.util.spec_from_file_location('qualified_rayraegah_dhash', S / 'dhash.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
D = module.dhash
rows = []
def check(name, action):
    result = action()
    rows.append(dict(name=name, passed=True, result=result))
def flat():
    a, b = Image.new('RGB', (9, 8), 'black'), Image.new('RGB', (9, 8), 'white')
    assert D.calculate_hash(a) == D.calculate_hash(b) == '0' * 16
    assert D.hamming_distance(a, b) == 0
    return 'Black and white have equal hash and zero image distance; no equality verdict.'
check('flat_black_white_collision', flat)
image = Image.new('L', (9, 8))
image.putdata([20, 10, 30, 40, 50, 60, 70, 80, 90] * 8)
def bit_order():
    assert D.calculate_hash(image) == '01' * 8
    return 'First descending comparison occupies low bit of each row byte.'
check('independent_first_bit_vector', bit_order)
def full_descending():
    other = Image.new('L', (9, 8))
    other.putdata(list(range(90, 0, -10)) * 8)
    assert D.calculate_hash(other) == 'f' * 16
    assert D.hamming_distance(image, other) == 56
    assert D.hamming_distance(D.calculate_hash(image), D.calculate_hash(other)) == 56
    return 56
check('all_bits_and_image_hash_distance_agree', full_descending)
def alpha():
    opaque = image.convert('RGBA')
    transparent = opaque.copy()
    transparent.putalpha(0)
    assert opaque.tobytes() != transparent.tobytes()
    assert D.calculate_hash(opaque) == D.calculate_hash(transparent)
    return 'Actual Pillow conversion drops alpha; hashes collide.'
check('different_alpha_same_hash', alpha)
def dimensions():
    assert D.calculate_hash(Image.new('L', (1, 1), 0)) == D.calculate_hash(Image.new('L', (180, 37), 255))
    return 'Different flat dimensions/values collide.'
check('flat_dimension_collision', dimensions)
def width():
    assert D.hamming_distance(hex(1 << 128), '0') == 1
    return 'Hash-string API accepts values wider than64bits.'
check('unbounded_hash_string_width', width)
def negative():
    assert D.hamming_distance('-1', '0') == 1
    return 'Negative hash strings accepted without domain validation.'
check('negative_hash_string_accepted', negative)
def bad_string():
    try:
        D.hamming_distance('not-hex', '0')
    except ValueError:
        return 'ValueError'
    raise AssertionError('malformed hex accepted')
check('malformed_hex_explicit_value_error', bad_string)
sha = lambda p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
out = R / 'docs/research/dedup-500/rayraegah-dhash-controls-20261013.json'
assert not out.exists()
out.write_text(json.dumps(dict(status='passed8_real_pillow_controls', controls=rows,
    python=sys.version, executable=sys.executable, pillow=PIL.__version__,
    upstream_tests_executed=0, pins={str(p):sha(p) for p in [S/'dhash.py', pathlib.Path(__file__), pathlib.Path(PIL.__file__)]},
    scope='Whole pinned original module, actual Pillow11.3.0, eight synthetic controls. No upstream dependency pins or tests exist. No general similarity threshold/precision or code adoption.'),indent=2)+'\n')
print('8 actual-Pillow controls passed')
