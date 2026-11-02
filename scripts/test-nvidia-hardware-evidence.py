import importlib.util
from pathlib import Path
import tempfile
import unittest
import hashlib
spec=importlib.util.spec_from_file_location('gate',Path(__file__).with_name('verify-nvidia-hardware-evidence.py'));gate=importlib.util.module_from_spec(spec);spec.loader.exec_module(gate)
class EvidenceTests(unittest.TestCase):
    def test_missing_stale_ambiguous_or_wrong_test_is_never_a_pass(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);src=root/'crates/rrrah-cuda/src';src.mkdir(parents=True);(src/'kernel.ptx').write_text('test fixture, not hardware evidence')
            manifest={'crates/rrrah-cuda/src/kernel.ptx':hashlib.sha256((src/'kernel.ptx').read_bytes()).hexdigest()}
            text=f'test {gate.TEST} ... ok\n{gate.MARKER}\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n'
            hw='0, GPU-fixture, fixture NVIDIA, 999.1\n'
            self.assertEqual(gate.verify(text,hw,root,manifest),[])
            for bad in [text.replace('1 passed','11 passed'),text.replace(gate.TEST,'other_test'),text.replace(gate.MARKER,''),text+gate.MARKER+'\n',text.replace('0 ignored','1 ignored'),text.replace('cases=42','cases=41'),text+'test result: ok. 0 passed; 0 failed; 0 ignored;\n']:
                self.assertTrue(gate.verify(bad,hw,root,manifest))
            self.assertTrue(gate.verify(text,'',root,manifest));self.assertTrue(gate.verify(text,hw,root,{}));(src/'kernel.ptx').write_text('changed');self.assertTrue(gate.verify(text,hw,root,manifest))
if __name__=='__main__':unittest.main()
