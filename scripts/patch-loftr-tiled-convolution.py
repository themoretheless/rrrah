"""Patch an isolated loftr 0.1.1 copy; preserve weight names and full image size."""
from pathlib import Path
import sys

root = Path(sys.argv[1])
p = root / 'src/backbone.rs'
s = p.read_text()
start = s.index('fn conv3x3(')
end = s.index('\n#[derive(Debug)]', start)
s = s[:start] + '''#[derive(Debug)]
struct TiledConv3x3 {
    conv: nn::Conv2D,
    stride: i64,
}

impl nn::Module for TiledConv3x3 {
    fn forward(&self, input: &Tensor) -> Tensor {
        // Include the global zero border once. Adjacent output strips read
        // overlapping input halos, so strip boundaries introduce no padding.
        let padded = input.constant_pad_nd([1, 1, 1, 1]);
        let output_height = (input.size()[2] - 1) / self.stride + 1;
        let mut strips = Vec::new();
        let mut first = 0;
        while first < output_height {
            let rows = (output_height - first).min(32);
            let strip = padded.narrow(2, first * self.stride, (rows - 1) * self.stride + 3);
            strips.push(strip.conv2d(
                &self.conv.ws, self.conv.bs.as_ref(),
                [self.stride, self.stride], [0, 0], [1, 1], 1,
            ));
            first += rows;
        }
        Tensor::cat(&strips, 2)
    }
}

fn conv3x3(vs: &nn::Path<'_>, in_planes: i64, out_planes: i64, stride: i64) -> TiledConv3x3 {
    TiledConv3x3 {
        conv: nn::conv2d(vs, in_planes, out_planes, 3, nn::ConvConfig {
            stride, padding: 1, bias: false, ..Default::default()
        }),
        stride,
    }
}
''' + s[end:]
# Only these two structs use 3x3 layers; the stem and 1x1 layers stay unchanged.
for struct in ('BasicBlock', 'FpnHead'):
    start = s.index('struct ' + struct + ' {')
    end = s.index('\n}', start)
    part = s[start:end]
    for field in ('conv1', 'conv2'):
        assert f'{field}: nn::Conv2D' in part
        part = part.replace(f'{field}: nn::Conv2D', f'{field}: TiledConv3x3')
    s = s[:start] + part + s[end:]
p.write_text(s)
p = root / 'src/backbone/tests.rs'
with p.open('a') as f:
    f.write('''
#[test]
fn tiled_convolution_matches_full_convolution_at_seams_and_borders() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    let _inference = tch::no_grad_guard();
    let vs = nn::VarStore::new(Device::Cpu);
    for (case, (batch, channels, height, width, stride)) in [
        (1, 3, 77, 81, 1), (2, 7, 78, 80, 2),
        (1, 128, 83, 97, 1), (1, 128, 137, 96, 2),
        (1, 3, 1, 1, 1), (1, 3, 2, 3, 2),
    ].into_iter().enumerate() {
        let layer = conv3x3(&(&vs.root() / case.to_string()), channels, 16, stride);
        let input = Tensor::randn([batch, channels, height, width], (Kind::Float, Device::Cpu));
        let expected = input.apply(&layer.conv);
        let actual = input.apply(&layer);
        assert_eq!(actual.size(), expected.size());
        let difference = (&actual - &expected).abs().max().double_value(&[]);
        assert!(actual.allclose(&expected, 1e-5, 1e-5, false), "case {case}: max error {difference}");
    }
    Ok(())
}
''')
