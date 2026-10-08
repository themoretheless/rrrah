use crate::font::Glyph;
use crate::soft_mask::SoftMask;
use crate::{BlendMode, ClipPath, Image};
use crate::{GlyphDrawMode, Paint, PathDrawMode};
use kurbo::{Affine, BezPath, Rect, Shape};

/// PDF transparency group properties, independent of group opacity and blend.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransparencyGroupProperties {
    /// Composite children against a transparent initial backdrop.
    pub isolated: bool,
    /// Composite each child's shape against the initial group backdrop.
    pub knockout: bool,
}

/// Declared group blending space, including unresolved explicit declarations.
#[derive(Debug, Clone, Default)]
pub enum TransparencyGroupColorSpace {
    /// No CS declaration; inherit from the enclosing group/output device.
    #[default]
    Inherited,
    /// Valid resolved device or calibrated blending space.
    Explicit(crate::color::ColorSpace),
    /// An explicit CS could not be resolved or is forbidden for blending.
    Unresolved,
}

/// A trait for a device that can be used to process PDF drawing instructions.
pub trait Device<'a> {
    /// Stop interpreting before the next operator, for cancellation or a fatal
    /// device allocation failure. The caller must discard incomplete output.
    /// Defaults to continuing for compatibility with existing devices.
    fn should_stop(&mut self) -> bool { false }

    /// Set the properties for future stroking operations.
    /// Interpret subsequent alpha constants and soft masks as shape when true.
    /// The PDF default is false (opacity); legacy devices may ignore this flag.
    fn set_alpha_source(&mut self, _alpha_is_shape: bool) {}
    /// Set a soft mask to be used for future drawing instructions.
    fn set_soft_mask(&mut self, mask: Option<SoftMask<'a>>);
    /// Set the blend mode that should be used for rendering operations.
    fn set_blend_mode(&mut self, blend_mode: BlendMode);
    /// Draw a path.
    fn draw_path(
        &mut self,
        path: &BezPath,
        transform: Affine,
        paint: &Paint<'a>,
        draw_mode: &PathDrawMode,
    );
    /// Push a new clip path to the clip stack.
    fn push_clip_path(&mut self, clip_path: &ClipPath);
    /// Push a new transparency group to the blend stack.
    fn push_transparency_group(
        &mut self,
        opacity: f32,
        mask: Option<SoftMask<'a>>,
        blend_mode: BlendMode,
    );
    /// Push a PDF group with its isolation and knockout properties.
    /// Devices implementing compositing should override this method. The
    /// default preserves compatibility for existing analysis-only devices.
    fn push_transparency_group_with_properties(
        &mut self,
        opacity: f32,
        mask: Option<SoftMask<'a>>,
        blend_mode: BlendMode,
        _properties: TransparencyGroupProperties,
    ) {
        self.push_transparency_group(opacity, mask, blend_mode);
    }
    /// Page group space. Rendering devices should preserve this before painting.
    fn set_page_group_color_space(&mut self, _space: &TransparencyGroupColorSpace) {}

    /// Carry the declared blending space without discarding legacy group flags.
    /// A compositor determines inheritance using the enclosing group/mask context.
    fn push_transparency_group_with_color_space(
        &mut self,
        opacity: f32,
        mask: Option<SoftMask<'a>>,
        blend_mode: BlendMode,
        properties: TransparencyGroupProperties,
        _space: &TransparencyGroupColorSpace,
    ) {
        self.push_transparency_group_with_properties(opacity, mask, blend_mode, properties);
    }
    /// Draw a glyph.
    fn draw_glyph(
        &mut self,
        glyph: &Glyph<'a>,
        transform: Affine,
        glyph_transform: Affine,
        paint: &Paint<'a>,
        // TODO: Move this into outline glyph.
        draw_mode: &GlyphDrawMode,
    );
    /// Draw an image.
    fn draw_image(&mut self, image: Image<'a, '_>, transform: Affine);
    /// Pop the last clip path from the clip stack.
    fn pop_clip_path(&mut self);
    /// Pop the last transparency group from the blend stack.
    fn pop_transparency_group(&mut self);
    /// Draw a rectangle directly, without going through the general path pipeline.
    fn draw_rect(
        &mut self,
        rect: &Rect,
        transform: Affine,
        paint: &Paint<'a>,
        draw_mode: &PathDrawMode,
    ) {
        self.draw_path(&rect.to_path(0.1), transform, paint, draw_mode);
    }
    /// Called at the beginning of a marked content sequence (BMC/BDC).
    ///
    /// The tag is the marked content tag (e.g. b"P", b"Span"). The mcid is
    /// the marked content identifier from the properties dict, if present.
    fn begin_marked_content(&mut self, _tag: &[u8], _mcid: Option<i32>) {}
    /// Called at the end of a marked content sequence (EMC).
    fn end_marked_content(&mut self) {}
}

/// A device that discards all drawing operations.
pub struct DummyDevice;

impl Device<'_> for DummyDevice {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'_>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
    fn draw_glyph(
        &mut self,
        _: &Glyph<'_>,
        _: Affine,
        _: Affine,
        _: &Paint<'_>,
        _: &GlyphDrawMode,
    ) {
    }
    fn draw_image(&mut self, _: Image<'_, '_>, _: Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}
