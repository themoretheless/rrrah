// Modified by rrrah (2026-10-10): strip-based 3x3 convolution and lazy debug statistics.
use tch::{
    Tensor,
    nn::{self},
};

use crate::{
    error::{LoftrError, check_cancel},
    loftr_config::{LoftrConfig, ResNetFpnConfig},
};

fn conv1x1(vs: &nn::Path<'_>, in_planes: i64, out_planes: i64, stride: i64) -> nn::Conv2D {
    nn::conv2d(
        vs,
        in_planes,
        out_planes,
        1,
        nn::ConvConfig {
            stride,
            padding: 0,
            bias: false,
            ..Default::default()
        },
    )
}

#[derive(Debug)]
struct TiledConv3x3 {
    conv: nn::Conv2D,
    stride: i64,
}

// Copy only the required rows and halo. Zero padding belongs exclusively
// to global image edges; an interior strip reads real neighboring rows.
fn padded_input_strip(
    input: &Tensor,
    first: i64,
    rows: i64,
    stride: i64,
) -> Result<Tensor, LoftrError> {
    let height = input.size()[2];
    let start = first * stride - 1;
    let end = (first + rows - 1) * stride + 2;
    let clipped_start = start.max(0);
    let clipped_end = end.min(height);
    Ok(input
        .f_narrow(2, clipped_start, clipped_end - clipped_start)?
        .f_constant_pad_nd([1, 1, (-start).max(0), (end - height).max(0)])?)
}

impl TiledConv3x3 {
    fn forward(&self, input: &Tensor, cancel: &dyn Fn() -> bool) -> Result<Tensor, LoftrError> {
        check_cancel(cancel)?;
        let output_height = (input.size()[2] - 1) / self.stride + 1;
        let mut strips = Vec::new();
        let mut first = 0;
        while first < output_height {
            check_cancel(cancel)?;
            let rows = (output_height - first).min(32);
            let strip = padded_input_strip(input, first, rows, self.stride)?;
            strips.push(strip.f_conv2d(
                &self.conv.ws,
                self.conv.bs.as_ref(),
                [self.stride, self.stride],
                [0, 0],
                [1, 1],
                1,
            )?);
            check_cancel(cancel)?;
            first += rows;
        }
        check_cancel(cancel)?;
        let output = Tensor::f_cat(&strips, 2)?;
        check_cancel(cancel)?;
        Ok(output)
    }
}

fn conv3x3(vs: &nn::Path<'_>, in_planes: i64, out_planes: i64, stride: i64) -> TiledConv3x3 {
    TiledConv3x3 {
        conv: nn::conv2d(
            vs,
            in_planes,
            out_planes,
            3,
            nn::ConvConfig {
                stride,
                padding: 1,
                bias: false,
                ..Default::default()
            },
        ),
        stride,
    }
}

#[derive(Debug)]
struct BasicBlock {
    conv1: TiledConv3x3,
    conv2: TiledConv3x3,
    bn1: nn::BatchNorm,
    bn2: nn::BatchNorm,
    downsample: Option<(nn::Conv2D, nn::BatchNorm)>,
}

impl BasicBlock {
    fn new(vs: &nn::Path<'_>, in_planes: i64, planes: i64, stride: i64) -> Self {
        let downsample = if stride == 1 {
            None
        } else {
            Some((
                conv1x1(&(vs / "downsample" / "0"), in_planes, planes, stride),
                nn::batch_norm2d(
                    &(vs / "downsample" / "1"),
                    planes,
                    nn::BatchNormConfig::default(),
                ),
            ))
        };
        Self {
            conv1: conv3x3(&(vs / "conv1"), in_planes, planes, stride),
            conv2: conv3x3(&(vs / "conv2"), planes, planes, 1),
            bn1: nn::batch_norm2d(&(vs / "bn1"), planes, nn::BatchNormConfig::default()),
            bn2: nn::batch_norm2d(&(vs / "bn2"), planes, nn::BatchNormConfig::default()),
            downsample,
        }
    }

    fn forward_t(
        &self,
        x: &Tensor,
        train: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Tensor, LoftrError> {
        check_cancel(cancel)?;
        let y = self
            .conv1
            .forward(x, cancel)?
            .apply_t(&self.bn1, train)
            .relu();
        let y = self.conv2.forward(&y, cancel)?.apply_t(&self.bn2, train);
        let residual = match &self.downsample {
            Some((conv, bn)) => x.apply(conv).apply_t(bn, train),
            None => x.shallow_clone(),
        };
        let output = (residual + y).relu();
        check_cancel(cancel)?;
        Ok(output)
    }
}

#[derive(Debug)]
struct ResNetLayer {
    block1: BasicBlock,
    block2: BasicBlock,
}

impl ResNetLayer {
    fn new(vs: &nn::Path<'_>, in_planes: i64, dim: i64, stride: i64) -> (Self, i64) {
        let block1 = BasicBlock::new(&(vs / "0"), in_planes, dim, stride);
        let block2 = BasicBlock::new(&(vs / "1"), dim, dim, 1);
        (Self { block1, block2 }, dim)
    }

    fn forward_t(
        &self,
        x: &Tensor,
        train: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Tensor, LoftrError> {
        let x = self.block1.forward_t(x, train, cancel)?;
        self.block2.forward_t(&x, train, cancel)
    }
}

#[derive(Debug)]
struct FpnHead {
    conv1: TiledConv3x3,
    bn: nn::BatchNorm,
    conv2: TiledConv3x3,
}

impl FpnHead {
    fn new(vs: &nn::Path<'_>, in_planes: i64, hidden_planes: i64, out_planes: i64) -> Self {
        Self {
            conv1: conv3x3(&(vs / "0"), in_planes, hidden_planes, 1),
            bn: nn::batch_norm2d(&(vs / "1"), hidden_planes, nn::BatchNormConfig::default()),
            conv2: conv3x3(&(vs / "3"), hidden_planes, out_planes, 1),
        }
    }

    fn forward_t(
        &self,
        x: &Tensor,
        train: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Tensor, LoftrError> {
        let x = self
            .conv1
            .forward(x, cancel)?
            .apply_t(&self.bn, train)
            .leaky_relu();
        self.conv2.forward(&x, cancel)
    }
}

#[derive(Debug)]
pub struct ResNetFpn8_2 {
    conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    layer1: ResNetLayer,
    layer2: ResNetLayer,
    layer3: ResNetLayer,
    layer3_outconv: nn::Conv2D,
    layer2_outconv: nn::Conv2D,
    layer2_outconv2: FpnHead,
    layer1_outconv: nn::Conv2D,
    layer1_outconv2: FpnHead,
}

impl ResNetFpn8_2 {
    pub fn new(vs: &nn::Path<'_>, config: &ResNetFpnConfig) -> Result<Self, LoftrError> {
        let initial_dim = config.initial_dim;
        let block_dims = config.block_dims;
        if initial_dim <= 0 || block_dims.iter().any(|dim| *dim <= 0) {
            return Err(LoftrError::InvalidConfig(format!(
                "ResNetFpn8_2 requires positive dimensions; got initial_dim={initial_dim}, block_dims={block_dims:?}"
            )));
        }

        let conv1 = nn::conv2d(
            &(vs / "conv1"),
            1,
            initial_dim,
            7,
            nn::ConvConfig {
                stride: 2,
                padding: 3,
                bias: false,
                ..Default::default()
            },
        );
        let bn1 = nn::batch_norm2d(&(vs / "bn1"), initial_dim, nn::BatchNormConfig::default());
        let (layer1, next_planes) =
            ResNetLayer::new(&(vs / "layer1"), initial_dim, block_dims[0], 1);
        let (layer2, next_planes) =
            ResNetLayer::new(&(vs / "layer2"), next_planes, block_dims[1], 2);
        let (layer3, _) = ResNetLayer::new(&(vs / "layer3"), next_planes, block_dims[2], 2);

        Ok(Self {
            conv1,
            bn1,
            layer1,
            layer2,
            layer3,
            layer3_outconv: conv1x1(&(vs / "layer3_outconv"), block_dims[2], block_dims[2], 1),
            layer2_outconv: conv1x1(&(vs / "layer2_outconv"), block_dims[1], block_dims[2], 1),
            layer2_outconv2: FpnHead::new(
                &(vs / "layer2_outconv2"),
                block_dims[2],
                block_dims[2],
                block_dims[1],
            ),
            layer1_outconv: conv1x1(&(vs / "layer1_outconv"), block_dims[0], block_dims[1], 1),
            layer1_outconv2: FpnHead::new(
                &(vs / "layer1_outconv2"),
                block_dims[1],
                block_dims[1],
                block_dims[0],
            ),
        })
    }

    pub fn forward_t(&self, x: &Tensor, train: bool) -> Result<(Tensor, Tensor), LoftrError> {
        self.forward_cancellable(x, train, &|| false)
    }

    /// Checks cancellation between backbone operations and convolution strips.
    pub fn forward_cancellable(
        &self,
        x: &Tensor,
        train: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(Tensor, Tensor), LoftrError> {
        check_cancel(cancel)?;
        let dims = x.size();
        if dims.len() != 4 || dims[1] != 1 {
            return Err(LoftrError::InvalidInput(format!(
                "ResNetFpn8_2 expects grayscale [N,1,H,W] input; got {dims:?}"
            )));
        }

        let x0 = x.apply(&self.conv1).apply_t(&self.bn1, train).relu();
        let x1 = self.layer1.forward_t(&x0, train, cancel)?;
        drop(x0);
        let x2 = self.layer2.forward_t(&x1, train, cancel)?;
        let x3 = self.layer3.forward_t(&x2, train, cancel)?;

        let x3_out = x3.apply(&self.layer3_outconv);
        drop(x3);
        let x2_out = x2.apply(&self.layer2_outconv);
        drop(x2);
        let x3_out_2x =
            x3_out.upsample_bilinear2d([x2_out.size()[2], x2_out.size()[3]], true, None, None);
        let x2_out = self
            .layer2_outconv2
            .forward_t(&(x2_out + x3_out_2x), train, cancel)?;

        let x1_out = x1.apply(&self.layer1_outconv);
        drop(x1);
        let x2_out_2x =
            x2_out.upsample_bilinear2d([x1_out.size()[2], x1_out.size()[3]], true, None, None);
        drop(x2_out);
        let x1_out = self
            .layer1_outconv2
            .forward_t(&(x1_out + x2_out_2x), train, cancel)?;

        check_cancel(cancel)?;
        Ok((x3_out, x1_out))
    }
}

#[derive(Debug)]
pub enum Backbone {
    ResNetFpn8_2(ResNetFpn8_2),
}

impl Backbone {
    pub fn forward_t(&self, x: &Tensor, train: bool) -> Result<(Tensor, Tensor), LoftrError> {
        self.forward_cancellable(x, train, &|| false)
    }

    /// Checks cancellation between backbone operations and convolution strips.
    pub fn forward_cancellable(
        &self,
        x: &Tensor,
        train: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(Tensor, Tensor), LoftrError> {
        check_cancel(cancel)?;
        match self {
            Self::ResNetFpn8_2(backbone) => backbone.forward_cancellable(x, train, cancel),
        }
    }
}

pub fn build_backbone(vs: &nn::Path<'_>, config: &LoftrConfig) -> Result<Backbone, LoftrError> {
    match config.backbone_type {
        crate::loftr_config::BackboneType::ResNetFpn => match config.resolution {
            (8, 2) => Ok(Backbone::ResNetFpn8_2(ResNetFpn8_2::new(
                &(vs / "backbone"),
                &config.resnetfpn,
            )?)),
            other => Err(LoftrError::InvalidConfig(format!(
                "Unsupported LoFTR backbone resolution {other:?}; only (8, 2) is ported so far"
            ))),
        },
    }
}

#[cfg(test)]
mod tests;
