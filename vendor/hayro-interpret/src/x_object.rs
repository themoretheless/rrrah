use crate::cache::Cache;
use crate::color::{ColorComponents, ColorSpace, ToRgb};
use crate::context::Context;
use crate::device::Device;
use crate::function::{Function, interpolate};
use crate::interpret::path::get_paint;
use crate::interpret::state::ActiveTransferFunction;
use crate::{BlendMode, CacheKey, ClipPath, Image, RasterImage, StencilImage};
use crate::{FillRule, InterpreterWarning, WarningSinkFn, interpret};
use crate::{ImageData, LumaData, RgbData};
use hayro_syntax::bit_reader::BitReader;
use hayro_syntax::content::TypedIter;
use hayro_syntax::object::Array;
use hayro_syntax::object::Dict;
use hayro_syntax::object::Name;
use hayro_syntax::object::Object;
use hayro_syntax::object::Stream;
use hayro_syntax::object::dict::keys::*;
use hayro_syntax::object::stream::{FilterResult, ImageColorSpace, ImageDecodeParams};
use hayro_syntax::page::Resources;
use kurbo::{Affine, Rect, Shape};
use smallvec::{SmallVec, smallvec};
use std::borrow::Cow;
use std::iter;
use std::ops::Deref;

pub(crate) enum XObject<'a> {
    FormXObject(FormXObject<'a>),
    ImageXObject(ImageXObject<'a>),
}

impl<'a> XObject<'a> {
    pub(crate) fn new(
        stream: &Stream<'a>,
        warning_sink: &WarningSinkFn,
        cache: &Cache,
        transfer_function: Option<ActiveTransferFunction>,
        defaults: &crate::color::DeviceColorDefaults,
        resolve_cs: impl FnOnce(&Name<'_>) -> Option<ColorSpace>,
    ) -> Option<Self> {
        let dict = stream.dict();
        match dict.get::<Name<'_>>(SUBTYPE)?.deref() {
            IMAGE => Some(Self::ImageXObject(ImageXObject::new(
                stream,
                resolve_cs,
                warning_sink,
                cache,
                false,
                transfer_function,
                defaults,
            )?)),
            FORM => Some(Self::FormXObject(FormXObject::new(stream)?)),
            _ => None,
        }
    }
}

pub(crate) struct FormXObject<'a> {
    pub(crate) decoded: Cow<'a, [u8]>,
    pub(crate) matrix: Affine,
    pub(crate) bbox: [f32; 4],
    is_transparency_group: bool,
    transparency_properties: crate::TransparencyGroupProperties,
    pub(crate) dict: Dict<'a>,
    resources: Dict<'a>,
}

impl<'a> FormXObject<'a> {
    pub(crate) fn new(stream: &Stream<'a>) -> Option<Self> {
        let dict = stream.dict();

        let decoded = stream.decoded().ok()?;
        let resources = dict.get::<Dict<'_>>(RESOURCES).unwrap_or_default();

        let matrix = Affine::new(
            dict.get::<[f64; 6]>(MATRIX)
                .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
        );
        let bbox = dict.get::<[f32; 4]>(BBOX)?;
        let is_transparency_group = dict.get::<Dict<'_>>(GROUP).is_some();
        let transparency_properties = transparency_group_properties(dict.get::<Dict<'_>>(GROUP).as_ref());

        Some(Self {
            decoded,
            matrix,
            is_transparency_group,
            transparency_properties,
            bbox,
            dict: dict.clone(),
            resources,
        })
    }
}

fn transparency_group_properties(group: Option<&Dict<'_>>) -> crate::TransparencyGroupProperties {
    crate::TransparencyGroupProperties {
        isolated: group.and_then(|group| group.get::<bool>(b"I")).unwrap_or(false),
        knockout: group.and_then(|group| group.get::<bool>(b"K")).unwrap_or(false),
    }
}

pub(crate) fn transparency_group_color_space(
    group: Option<&Dict<'_>>,
    context: &Context<'_>,
    resources: &Resources<'_>,
) -> crate::TransparencyGroupColorSpace {
    use crate::TransparencyGroupColorSpace as State;
    let Some(group) = group else { return State::Inherited; };
    if !group.contains_key(CS) { return State::Inherited; }
    let resolved = group.get::<Object<'_>>(CS).and_then(|object| match object {
        Object::Name(name) => ColorSpace::new_from_name(&name)
            .or_else(|| context.get_color_space(resources, &name)),
        object => ColorSpace::new(object, &context.interpreter_cache.object_cache),
    }).map(|space| space.with_device_defaults(&context.device_color_defaults(resources)))
      .filter(ColorSpace::valid_blending_space);
    resolved.map_or(State::Unresolved, State::Explicit)
}

#[cfg(test)]
mod transparency_group_properties_tests {
    use super::*;
    use hayro_syntax::object::FromBytes;

    #[test]
    fn preserves_independent_isolation_and_knockout_flags() {
        for (bytes, isolated, knockout) in [
            (b"<< /S /Transparency /I false /K true >>".as_slice(), false, true),
            (b"<< /S /Transparency /I true /K false >>".as_slice(), true, false),
            (b"<< /S /Transparency /I true /K true >>".as_slice(), true, true),
            (b"<< /S /Transparency >>".as_slice(), false, false),
        ] {
            let dict = Dict::from_bytes(bytes).unwrap();
            assert_eq!(transparency_group_properties(Some(&dict)), crate::TransparencyGroupProperties { isolated, knockout });
        }
        assert_eq!(transparency_group_properties(None), crate::TransparencyGroupProperties::default());
    }
}

pub(crate) fn draw_xobject<'a>(
    x_object: &XObject<'a>,
    resources: &Resources<'a>,
    context: &mut Context<'a>,
    device: &mut impl Device<'a>,
) {
    match x_object {
        XObject::FormXObject(f) => draw_form_xobject(resources, f, context, device),
        XObject::ImageXObject(i) => {
            draw_image_xobject(i, context, device);
        }
    }
}

pub(crate) fn draw_form_xobject<'a, 'b>(
    resources: &Resources<'a>,
    x_object: &'b FormXObject<'a>,
    context: &mut Context<'a>,
    device: &mut impl Device<'a>,
) {
    if !context.ocg_state.is_visible() {
        return;
    }

    if !context.begin_nested_interpretation() {
        return;
    }

    let has_oc = xobject_oc(&x_object.dict, context);
    if !context.ocg_state.is_visible() {
        if has_oc {
            context.ocg_state.end_marked_content();
        }
        context.end_nested_interpretation();
        return;
    }

    let iter = TypedIter::new(x_object.decoded.as_ref());

    context.path_mut().truncate(0);
    context.save_state();
    context.pre_concat_affine(x_object.matrix);
    context.push_root_transform();

    if x_object.is_transparency_group {
        device.set_alpha_source(context.get().graphics_state.alpha_is_shape);
        let local_resources = Resources::from_parent(x_object.resources.clone(), resources.clone());
        let color_space = transparency_group_color_space(
            x_object.dict.get::<Dict<'_>>(GROUP).as_ref(), context, &local_resources,
        );
        device.push_transparency_group_with_color_space(
            context.get().graphics_state.non_stroke_alpha,
            std::mem::take(&mut context.get_mut().graphics_state.soft_mask),
            std::mem::take(&mut context.get_mut().graphics_state.blend_mode),
            x_object.transparency_properties,
            &color_space,
        );

        context.get_mut().graphics_state.non_stroke_alpha = 1.0;
        context.get_mut().graphics_state.stroke_alpha = 1.0;
    }

    device.set_alpha_source(context.get().graphics_state.alpha_is_shape);
    device.set_soft_mask(context.get().graphics_state.soft_mask.clone());
    device.set_blend_mode(context.get().graphics_state.blend_mode);

    device.push_clip_path(&ClipPath {
        path: context.get().ctm
            * Rect::new(
                x_object.bbox[0] as f64,
                x_object.bbox[1] as f64,
                x_object.bbox[2] as f64,
                x_object.bbox[3] as f64,
            )
            .to_path(0.1),
        fill: FillRule::NonZero,
    });

    interpret(
        iter,
        &Resources::from_parent(x_object.resources.clone(), resources.clone()),
        context,
        device,
    );

    device.pop_clip_path();

    if x_object.is_transparency_group {
        device.pop_transparency_group();
    }

    context.pop_root_transform();
    context.restore_state(device);

    if has_oc {
        context.ocg_state.end_marked_content();
    }

    context.end_nested_interpretation();
}

pub(crate) fn draw_image_xobject<'a, 'b>(
    x_object: &ImageXObject<'b>,
    context: &mut Context<'a>,
    device: &mut impl Device<'a>,
) {
    if !context.ocg_state.is_visible() {
        return;
    }

    let has_oc = xobject_oc(x_object.stream.dict(), context);
    if !context.ocg_state.is_visible() {
        if has_oc {
            context.ocg_state.end_marked_content();
        }
        return;
    }

    let width = x_object.width as f64;
    let height = x_object.height as f64;

    context.save_state();
    context.pre_concat_affine(Affine::new([
        1.0 / width,
        0.0,
        0.0,
        -1.0 / height,
        0.0,
        1.0,
    ]));
    let transform = context.get().ctm;

    let has_alpha = x_object.has_mask();

    let mut soft_mask = std::mem::take(&mut context.get_mut().graphics_state.soft_mask);
    let blend_mode = std::mem::take(&mut context.get_mut().graphics_state.blend_mode);

    // If image has smask, the soft mask from the graphics state should be discarde.
    if has_alpha {
        soft_mask = None;
    }

    device.set_alpha_source(context.get().graphics_state.alpha_is_shape);
    device.push_transparency_group(
        context.get().graphics_state.non_stroke_alpha,
        std::mem::take(&mut soft_mask),
        blend_mode,
    );

    // Image opacity belongs to the surrounding group. Stencil paint must not
    // apply the same graphics-state alpha a second time. restore_state below
    // restores the caller's alpha after the image has been drawn.
    context.get_mut().graphics_state.non_stroke_alpha = 1.0;
    context.get_mut().graphics_state.stroke_alpha = 1.0;

    device.set_alpha_source(context.get().graphics_state.alpha_is_shape);
    device.set_soft_mask(None);
    device.set_blend_mode(BlendMode::default());

    let image = if x_object.is_mask {
        Image::Stencil(StencilImage {
            paint: get_paint(context, false),
            image_xobject: x_object.clone(),
        })
    } else {
        Image::Raster(RasterImage(x_object.clone()))
    };

    device.draw_image(image, transform);
    device.pop_transparency_group();

    context.restore_state(device);

    if has_oc {
        context.ocg_state.end_marked_content();
    }
}

fn xobject_oc(dict: &Dict<'_>, context: &mut Context<'_>) -> bool {
    let Some(oc_dict) = dict.get::<Dict<'_>>(OC) else {
        return false;
    };

    if let Some(oc_ref) = dict.get_ref(OC) {
        context.ocg_state.begin_ocg(&oc_dict, oc_ref.into());
    } else {
        context.ocg_state.begin_ocmd(&oc_dict);
    }

    true
}

#[derive(Clone)]
pub(crate) struct ImageXObject<'a> {
    width: u32,
    height: u32,
    color_space: Option<ColorSpace>,
    cache: Cache,
    interpolate: bool,
    is_mask: bool,
    is_stencil_mask: bool,
    stream: Stream<'a>,
    transfer_function: Option<ActiveTransferFunction>,
    warning_sink: WarningSinkFn,
    defaults: crate::color::DeviceColorDefaults,
}

impl<'a> ImageXObject<'a> {
    pub(crate) fn new(
        stream: &Stream<'a>,
        resolve_cs: impl FnOnce(&Name<'_>) -> Option<ColorSpace>,
        warning_sink: &WarningSinkFn,
        cache: &Cache,
        mut is_mask: bool,
        transfer_function: Option<ActiveTransferFunction>,
        defaults: &crate::color::DeviceColorDefaults,
    ) -> Option<Self> {
        let dict = stream.dict();

        let is_stencil_mask = dict
            .get::<bool>(IM)
            .or_else(|| dict.get::<bool>(IMAGE_MASK))
            .unwrap_or(false);
        is_mask |= is_stencil_mask;

        let image_cs = if is_mask {
            // Masks are always single-channel.
            Some(ColorSpace::device_gray())
        } else {
            let cs_obj = dict
                .get::<Object<'_>>(CS)
                .or_else(|| dict.get::<Object<'_>>(COLORSPACE));

            cs_obj
                .clone()
                .and_then(|c| ColorSpace::new(c, cache))
                // Inline images can also refer to color spaces by name.
                .or_else(|| {
                    cs_obj
                        .and_then(|c| c.into_name())
                        .and_then(|n| resolve_cs(&n))
                })
        };

        let interpolate = dict
            .get::<bool>(I)
            .or_else(|| dict.get::<bool>(INTERPOLATE))
            .unwrap_or(false);

        let width = dict.get::<u32>(W).or_else(|| dict.get::<u32>(WIDTH))?;
        let height = dict.get::<u32>(H).or_else(|| dict.get::<u32>(HEIGHT))?;

        if width == 0 || height == 0 {
            return None;
        }

        Some(Self {
            width,
            cache: cache.clone(),
            height,
            color_space: image_cs.map(|cs| if is_mask { cs } else { cs.with_device_defaults(defaults) }),
            defaults: if is_mask { Default::default() } else { defaults.clone() },
            warning_sink: warning_sink.clone(),
            transfer_function,
            interpolate,
            stream: stream.clone(),
            is_mask,
            is_stencil_mask,
        })
    }

    pub(crate) fn decoded_mask(&self, target_dimension: Option<(u32, u32)>, cancelled: &dyn Fn() -> bool) -> Option<DecodedMask> {
        if !self.is_mask {
            return None;
        }

        decode_mask(self, target_dimension, cancelled)
    }

    pub(crate) fn native_components(
        &self, cancelled: &dyn Fn() -> bool,
        admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    ) -> Result<crate::types::NativeRasterComponents,crate::types::NativeRasterComponentsError> {
        use crate::types::NativeRasterComponentsError as Error;
        let latched = latch_cancellation(cancelled);
        let cancelled: &dyn Fn() -> bool = &latched;
        if cancelled() { return Err(Error::Cancelled); }
        let soft_mask = if self.is_mask || self.has_mask() {
            use crate::types::NativeRasterMaskKind as Kind;
            let dict=self.stream.dict();
            let kind=if self.is_mask { Kind::Stencil }
                else if dict.get::<u8>(SMASK_IN_DATA).is_some_and(|value| value==1 || value==2) { Kind::Embedded }
                else if dict.get::<Stream<'_>>(SMASK).is_some() { Kind::Soft }
                else if dict.get::<Stream<'_>>(MASK).is_some() { Kind::Explicit }
                else if dict.get::<Array<'_>>(MASK).is_some() { Kind::ColorKey }
                else { Kind::Unsupported };
            if kind==Kind::Soft {
                let stream=dict.get::<Stream<'_>>(SMASK).ok_or(Error::Mask(kind))?;
                if stream.dict().contains_key(MATTE) { return Err(Error::Mask(Kind::SoftMatte)); }
                Some(stream)
            } else { return Err(Error::Mask(kind)); }
        } else { None };
        if self.transfer_function.is_some() { return Err(Error::TransferFunction); }
        let credit = admit(size_of::<crate::types::NativeRasterComponents>()).ok_or(Error::Admission)?;
        if cancelled() { return Err(Error::Cancelled); }
        let ctx = decode_context(self,None).ok_or(Error::Decode)?;
        if cancelled() { return Err(Error::Cancelled); }
        if ctx.scale_factors!=(1.,1.) { return Err(Error::Scaled); }
        let denied = std::cell::Cell::new(false);
        let samples = crate::native_image_samples::unpack(&ctx.decoded.data,ctx.width,ctx.height,
            ctx.color_space.num_components(),ctx.bits_per_component,&ctx.decode_arr,cancelled,&|bytes| {
                let value=admit(bytes); if value.is_none() { denied.set(true); } value
            }).ok_or_else(|| if cancelled() { Error::Cancelled } else if denied.get() { Error::Admission } else { Error::ComponentData })?;
        if cancelled() { return Err(Error::Cancelled); }
        let alpha = if let Some(stream)=soft_mask {
            let alpha_credit=admit(size_of::<crate::types::NativeRasterAlpha>()).ok_or(Error::Admission)?;
            if cancelled() { return Err(Error::Cancelled); }
            let mask=ImageXObject::new(&stream,|_|None,&self.warning_sink,&self.cache,true,None,&Default::default()).ok_or(Error::Decode)?;
            if mask.is_stencil_mask { return Err(Error::ComponentData); }
            let mask_ctx=decode_context(&mask,None).ok_or(Error::Decode)?;
            if cancelled() { return Err(Error::Cancelled); }
            if mask_ctx.color_space.num_components()!=1 || mask_ctx.scale_factors!=(1.,1.) { return Err(Error::ComponentData); }
            denied.set(false);
            let mut values=crate::native_image_samples::unpack(&mask_ctx.decoded.data,mask_ctx.width,mask_ctx.height,1,
                mask_ctx.bits_per_component,&mask_ctx.decode_arr,cancelled,&|bytes| {
                    let value=admit(bytes);if value.is_none() {denied.set(true);}value
                }).ok_or_else(||if cancelled(){Error::Cancelled}else if denied.get(){Error::Admission}else{Error::ComponentData})?;
            values.clamp_alpha(cancelled).ok_or(Error::Cancelled)?;
            Some(crate::types::NativeRasterAlpha {width:mask_ctx.width,height:mask_ctx.height,interpolate:mask.interpolate,samples:values,_credit:alpha_credit})
        } else {None};
        if cancelled() { return Err(Error::Cancelled); }
        Ok(crate::types::NativeRasterComponents {color_space:ctx.color_space,width:ctx.width,height:ctx.height,
            interpolate:self.interpolate,alpha,samples,_credit:credit})
    }

    pub(crate) fn decoded_raster(
        &self,
        target_dimension: Option<(u32, u32)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<DecodedRaster> {
        if self.is_mask {
            return None;
        }

        decode_raster(self, target_dimension, cancelled)
    }

    pub(crate) fn width(&self) -> u32 {
        self.width
    }

    pub(crate) fn height(&self) -> u32 {
        self.height
    }

    pub(crate) fn color_space(&self) -> Option<&ColorSpace> { self.color_space.as_ref() }

    pub(crate) fn stream(&self) -> &Stream<'a> {
        &self.stream
    }

    fn has_mask(&self) -> bool {
        let dict = self.stream.dict();

        dict.contains_key(SMASK_IN_DATA) || dict.contains_key(SMASK) || dict.contains_key(MASK)
    }
}

pub(crate) struct DecodedMask {
    pub(crate) luma: LumaData,
}

pub(crate) struct DecodedRaster {
    pub(crate) image: ImageData,
    pub(crate) alpha: Option<LumaData>,
}

struct DecodeContext<'a> {
    decoded: FilterResult<'a>,
    width: u32,
    height: u32,
    scale_factors: (f32, f32),
    color_space: ColorSpace,
    bits_per_component: u8,
    decode_arr: SmallVec<[(f32, f32); 4]>,
}

fn decode_context<'a>(
    obj: &ImageXObject<'a>,
    target_dimension: Option<(u32, u32)>,
) -> Option<DecodeContext<'a>> {
    let dict = obj.stream.dict();
    let dict_bpc = dict
        .get::<u8>(BPC)
        .or_else(|| dict.get::<u8>(BITS_PER_COMPONENT));
    let color_space = obj.color_space.clone();
    let is_indexed = obj.color_space.as_ref().is_some_and(|cs| cs.is_indexed());

    let decode_params = ImageDecodeParams {
        is_indexed,
        bpc: dict_bpc,
        num_components: color_space.as_ref().map(|c| c.num_components()),
        target_dimension,
        width: obj.width,
        height: obj.height,
    };

    let decoded = obj
        .stream
        .decoded_image(&decode_params)
        .map_err(|_| (obj.warning_sink)(InterpreterWarning::ImageDecodeFailure))
        .ok()?;

    let (mut scale_x, mut scale_y) = (1.0, 1.0);

    let (width, height) = decoded
        .image_data
        .as_ref()
        .map(|d| {
            scale_x = obj.width as f32 / d.width as f32;
            scale_y = obj.height as f32 / d.height as f32;

            (d.width, d.height)
        })
        .unwrap_or((obj.width, obj.height));

    let color_space = color_space
        .or_else(|| {
            decoded
                .image_data
                .as_ref()
                .map(|i| i.color_space)
                .and_then(|c| {
                    c.and_then(|c| match c {
                        ImageColorSpace::Gray => Some(ColorSpace::device_gray()),
                        ImageColorSpace::Rgb => Some(ColorSpace::device_rgb()),
                        ImageColorSpace::Cmyk => Some(ColorSpace::device_cmyk()),
                        ImageColorSpace::Unknown(_) => None,
                    })
                })
        })
        .unwrap_or(ColorSpace::device_gray()).with_device_defaults(&obj.defaults);

    let fallback_bpc = if obj.is_stencil_mask { 1 } else { 8 };

    let bits_per_component = decoded
        .image_data
        .as_ref()
        .map(|i| i.bits_per_component)
        .or(dict_bpc)
        .unwrap_or(fallback_bpc);

    let decode_arr = dict
        .get::<Array<'_>>(D)
        .or_else(|| dict.get::<Array<'_>>(DECODE))
        .map(|a| a.iter::<(f32, f32)>().collect::<SmallVec<_>>())
        .unwrap_or(color_space.default_decode_arr(bits_per_component as f32));

    Some(DecodeContext {
        decoded,
        width,
        height,
        scale_factors: (scale_x, scale_y),
        color_space,
        bits_per_component,
        decode_arr,
    })
}

fn decode_mask(
    obj: &ImageXObject<'_>,
    target_dimension: Option<(u32, u32)>,
    cancelled: &dyn Fn() -> bool,
) -> Option<DecodedMask> {
    if cancelled() { return None; }
    let ctx = decode_context(obj, target_dimension)?;
    if cancelled() { return None; }
    let mut height = ctx.height;

    let data = decode_mask_bytes(
        ctx.decoded.data,
        ctx.width,
        &mut height,
        &ctx.color_space,
        ctx.bits_per_component,
        &ctx.decode_arr,
        // Note: The semantics between "normal" soft masks (i.e. masks defined in
        // the graphics state or via `Mask`/`SMask` are inverted compared to
        // stencil masks (defined via `ImageMask`). The former match the semantics
        // of normal alpha images, where 0 stands for invisible and MAX stands for
        // fully opaque. For stencil masks, it's the other way around: 1 means the
        // paint is visible, while 0 means it's invisible.
        obj.is_stencil_mask,
        cancelled,
    )?;

    Some(DecodedMask {
        luma: LumaData {
            data,
            width: ctx.width,
            height,
            interpolate: obj.interpolate,
            scale_factors: ctx.scale_factors,
        },
    })
}

fn decode_raster(
    obj: &ImageXObject<'_>,
    target_dimension: Option<(u32, u32)>,
    cancelled: &dyn Fn() -> bool,
) -> Option<DecodedRaster> {
    let latched = latch_cancellation(cancelled);
    let cancelled: &dyn Fn() -> bool = &latched;
    if cancelled() { return None; }
    let mut ctx = decode_context(obj, target_dimension)?;
    if cancelled() { return None; }
    let mut height = ctx.height;

    let is_default_decode = ctx.decode_arr
        == ctx
            .color_space
            .default_decode_arr(ctx.bits_per_component as f32);
    let is_inverted_default_decode = ctx.decode_arr
        == ctx
            .color_space
            .inverted_default_decode_arr(ctx.bits_per_component as f32);

    let image_data = if ctx.bits_per_component == 8
        && ctx.color_space.supports_u8()
        && obj.transfer_function.is_none()
        && (is_default_decode || is_inverted_default_decode)
    {
        // This is actually the most common case, where the PDF is embedded
        // in such a way where we don't need to decode. In this case,
        // we can prevent the round-trip from f32 back to u8 and just return
        // the raw decoded data, which will already be in
        // RGB8/gray-scale with values between 0 and 255.
        fix_image_length(
            ctx.decoded.data.to_mut(),
            ctx.width,
            &mut height,
            0,
            &ctx.color_space,
        )?;

        if is_inverted_default_decode {
            if cancelled() { return None; }
            invert_u8_with_cancel(ctx.decoded.data.to_mut(), cancelled)?;
        }

        if ctx.color_space.is_device_gray() {
            Some(ImageData::Luma(LumaData {
                data: core::mem::take(&mut ctx.decoded.data).into_owned(),
                width: ctx.width,
                height,
                interpolate: obj.interpolate,
                scale_factors: ctx.scale_factors,
            }))
        } else {
            let output_buf = get_rgb_u8_data(&ctx.decoded.data, ctx.width, height, &ctx.color_space, cancelled)?;

            Some(ImageData::Rgb(RgbData {
                data: output_buf,
                width: ctx.width,
                height,
                interpolate: obj.interpolate,
                scale_factors: ctx.scale_factors,
            }))
        }
    } else {
        let components = get_components(
            &ctx.decoded.data,
            ctx.width,
            height,
            &ctx.color_space,
            ctx.bits_per_component,
        )?;

        let mut f32_data = apply_decode_array(
            &components,
            &ctx.color_space,
            ctx.bits_per_component,
            &ctx.decode_arr,
        )?;

        fix_image_length(&mut f32_data, ctx.width, &mut height, 0.0, &ctx.color_space)?;

        let mut rgb_data = get_rgb_data(
            &f32_data,
            ctx.width,
            height,
            ctx.scale_factors,
            &ctx.color_space,
            obj.interpolate,
            cancelled,
        );

        if let Some(transfer_function) = &obj.transfer_function
            && let Some(rgb_data) = &mut rgb_data
        {
            let apply_single = |data: u8, function: &Function| {
                function
                    .eval(smallvec![data as f32 / 255.0])
                    .and_then(|v| v.first().copied())
                    .map(|v| (v * 255.0 + 0.5) as u8)
                    .unwrap_or(data)
            };

            match transfer_function {
                ActiveTransferFunction::Single(s) => {
                    let mut table = [None; 256];
                    for (index, data) in rgb_data.data.iter_mut().enumerate() {
                        if index % 1024 == 0 && cancelled() { return None; }
                        *data = transfer_lookup(&mut table, *data, |value| apply_single(value, s));
                    }
                }
                ActiveTransferFunction::Four(f) => {
                    let mut tables = [[None; 256]; 3];
                    for (index, data) in rgb_data.data.chunks_exact_mut(3).enumerate() {
                        if index % 1024 == 0 && cancelled() { return None; }
                        data[0] = transfer_lookup(&mut tables[0], data[0], |value| apply_single(value, &f[0]));
                        data[1] = transfer_lookup(&mut tables[1], data[1], |value| apply_single(value, &f[1]));
                        data[2] = transfer_lookup(&mut tables[2], data[2], |value| apply_single(value, &f[2]));
                    }
                }
            }
        }

        rgb_data.map(ImageData::Rgb)
    };

    let mut image = image_data?;
    if cancelled() { return None; }

    let alpha = if let Some((alpha, matte_rgb)) =
        resolve_matte(obj, &ctx.color_space, target_dimension, cancelled)
        && alpha.width == ctx.width
        && alpha.height == height
    {
        unpremultiply(&mut image, &alpha.data, &matte_rgb, cancelled)?;

        Some(alpha)
    } else {
        // Use flatten here, so in case the alpha channel is invalid we can still
        // return the main image (see PDFJS-19611).
        resolve_alpha(
            obj,
            &mut ctx.decoded,
            Some(&image),
            &ctx.color_space,
            ctx.bits_per_component,
            ctx.width,
            &mut height,
            ctx.scale_factors,
            target_dimension,
            cancelled,
        )
        .flatten()
    };

    if cancelled() { return None; }
    Some(DecodedRaster { image, alpha })
}

fn decode_mask_bytes(
    mut decoded_data: Cow<'_, [u8]>,
    width: u32,
    height: &mut u32,
    color_space: &ColorSpace,
    bits_per_component: u8,
    decode_arr: &[(f32, f32)],
    invert: bool,
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<u8>> {
    if cancelled() { return None; }
    let default_decode = color_space.default_decode_arr(bits_per_component as f32);
    let inverted_default = color_space.inverted_default_decode_arr(bits_per_component as f32);
    let fast_path = bits_per_component == 8
        && (decode_arr == default_decode.as_slice() || decode_arr == inverted_default.as_slice());

    let mut data = if fast_path {
        let should_invert = invert ^ (decode_arr == inverted_default.as_slice());
        if should_invert {
            if cancelled() { return None; }
            invert_u8_with_cancel(decoded_data.to_mut(), cancelled)?;
        }

        decoded_data.into_owned()
    } else {
        let components = get_components(
            &decoded_data,
            width,
            *height,
            color_space,
            bits_per_component,
        )?;

        let f32_data =
            apply_decode_array(&components, color_space, bits_per_component, decode_arr)?;

        let mut result = Vec::with_capacity(f32_data.len());
        for (index, alpha) in f32_data.iter().enumerate() {
            if index % 1024 == 0 && cancelled() { return None; }
            result.push(if invert { ((1.0 - *alpha) * 255.0 + 0.5) as u8 }
                else { (*alpha * 255.0 + 0.5) as u8 });
        }
        result
    };

    if cancelled() { return None; }
    fix_image_length(&mut data, width, height, 0, color_space)?;
    if cancelled() { return None; }
    Some(data)
}

fn resolve_alpha(
    obj: &ImageXObject<'_>,
    decoded: &mut FilterResult<'_>,
    image_data: Option<&ImageData>,
    color_space: &ColorSpace,
    bits_per_component: u8,
    width: u32,
    height: &mut u32,
    scale_factors: (f32, f32),
    target_dimension: Option<(u32, u32)>,
    cancelled: &dyn Fn() -> bool,
) -> Option<Option<LumaData>> {
    if cancelled() { return None; }
    let dict = obj.stream.dict();

    let alpha = if let Some(1) = dict.get::<u8>(SMASK_IN_DATA) {
        let smask_data = decoded.image_data.as_mut().and_then(|i| i.alpha.take());

        if let Some(mut data) = smask_data {
            fix_image_length(&mut data, width, height, 0, &ColorSpace::device_gray())?;

            Some(LumaData {
                data,
                width,
                height: *height,
                interpolate: obj.interpolate,
                scale_factors,
            })
        } else {
            None
        }
        // Note: `SMASK` field takes precedence over `MASK`, so order matters here.
    } else if let Some(s_mask) = dict
        .get::<Stream<'_>>(SMASK)
        .or_else(|| dict.get::<Stream<'_>>(MASK))
    {
        let obj = ImageXObject::new(&s_mask, |_| None, &obj.warning_sink, &obj.cache, true, None, &Default::default())?;

        decode_mask(&obj, target_dimension, cancelled).map(|decoded| decoded.luma)
    } else if let Some(color_key_mask) = dict.get::<SmallVec<[u16; 4]>>(MASK) {
        let mut mask_data = vec![];

        // TODO: Make this less ugly.
        let raw_data = match image_data {
            Some(ImageData::Luma(d)) => &d.data,
            _ => decoded.data.as_ref(),
        };

        let components = get_components(raw_data, width, *height, color_space, bits_per_component)?;

        for (index, pixel) in components.chunks_exact(color_space.num_components() as usize).enumerate() {
            if index % 1024 == 0 && cancelled() { return None; }
            let mut mask_val = 0;

            for (component, min_max) in pixel.iter().zip(color_key_mask.chunks_exact(2)) {
                if *component > min_max[1] || *component < min_max[0] {
                    mask_val = 255;
                }
            }

            mask_data.push(mask_val);
        }

        fix_image_length(&mut mask_data, width, height, 0, &ColorSpace::device_gray())?;

        Some(LumaData {
            data: mask_data,
            width,
            height: *height,
            interpolate: obj.interpolate,
            scale_factors,
        })
    } else {
        None
    };

    Some(alpha)
}

fn resolve_matte(
    obj: &ImageXObject<'_>,
    color_space: &ColorSpace,
    target_dimension: Option<(u32, u32)>,
    cancelled: &dyn Fn() -> bool,
) -> Option<(LumaData, [u8; 3])> {
    let dict = obj.stream.dict();
    let s_mask = dict.get::<Stream<'_>>(SMASK)?;
    let matte = s_mask.dict().get::<ColorComponents>(MATTE)?;

    if matte.len() != color_space.num_components() as usize {
        return None;
    }

    // In theory, matte needs to be applied in the image's original color space,
    // but we always do it in RGB for now.
    let mut matte_rgb = [0_u8; 3];
    color_space.convert_f32(&matte, &mut matte_rgb, false);

    let mask_obj = ImageXObject::new(&s_mask, |_| None, &obj.warning_sink, &obj.cache, true, None, &Default::default())?;
    let alpha = decode_mask(&mask_obj, target_dimension, cancelled)?.luma;

    Some((alpha, matte_rgb))
}

fn unpremultiply(image: &mut ImageData, alpha: &[u8], matte_rgb: &[u8], cancelled: &dyn Fn() -> bool) -> Option<()> {
    if cancelled() { return None; }
    match image {
        ImageData::Rgb(rgb) => {
            for (index, (pixel, &a)) in rgb.data.chunks_exact_mut(3).zip(alpha.iter()).enumerate() {
                if index % 1024 == 0 && cancelled() { return None; }
                if a == 0 {
                    continue;
                }
                let inv_alpha = 255.0 / a as f32;
                for (c, &m) in pixel.iter_mut().zip(matte_rgb.iter()) {
                    let m = m as f32;
                    *c = (m + (*c as f32 - m) * inv_alpha) as u8;
                }
            }
        }
        ImageData::Luma(luma) => {
            let m = matte_rgb[0] as f32;
            for (index, (c, &a)) in luma.data.iter_mut().zip(alpha.iter()).enumerate() {
                if index % 1024 == 0 && cancelled() { return None; }
                if a == 0 {
                    continue;
                }
                let inv_alpha = 255.0 / a as f32;
                *c = (m + (*c as f32 - m) * inv_alpha) as u8;
            }
        }
    }
    if cancelled() { return None; }
    Some(())
}

// Optional mask failures may fall back to another decode route. Cancellation
// must survive that fallback even when the caller emits a one-shot signal.
fn latch_cancellation(cancelled: &dyn Fn() -> bool) -> impl Fn() -> bool + '_ {
    let seen = std::cell::Cell::new(false);
    move || {
        if seen.get() { return true; }
        let value = cancelled();
        seen.set(value);
        value
    }
}

fn invert_u8_with_cancel(data: &mut [u8], cancelled: &dyn Fn() -> bool) -> Option<()> {
    if cancelled() { return None; }
    for block in data.chunks_mut(1024) {
        if cancelled() { return None; }
        for value in block { *value = 255 - *value; }
    }
    if cancelled() { return None; }
    Some(())
}

// Evaluate only values present in the image, at most once per channel.
fn transfer_lookup(table: &mut [Option<u8>; 256], value: u8, evaluate: impl FnOnce(u8) -> u8) -> u8 {
    *table[value as usize].get_or_insert_with(|| evaluate(value))
}

fn get_rgb_u8_data(decoded: &[u8], width: u32, height: u32, cs: &ColorSpace, cancelled: &dyn Fn() -> bool) -> Option<Vec<u8>> {
    let components = cs.num_components() as usize;
    if components == 0 || cancelled() { return None; }
    let mut output = vec![0; width as usize * height as usize * 3];
    for (source, target) in decoded.chunks(1024 * components).zip(output.chunks_mut(1024 * 3)) {
        if cancelled() { return None; }
        cs.convert_u8(source, target)?;
    }
    if cancelled() { return None; }
    Some(output)
}

fn get_rgb_data(
    decoded: &[f32],
    width: u32,
    height: u32,
    scale_factors: (f32, f32),
    cs: &ColorSpace,
    interpolate: bool,
    cancelled: &dyn Fn() -> bool,
) -> Option<RgbData> {
    // To prevent a panic when calling the `chunks` method.
    if cs.num_components() == 0 {
        return None;
    }

    if cancelled() { return None; }
    let mut output = vec![0; width as usize * height as usize * 3];
    let components = cs.num_components() as usize;
    for (source, target) in decoded.chunks(1024 * components).zip(output.chunks_mut(1024 * 3)) {
        if cancelled() { return None; }
        cs.convert_f32(source, target, false);
    }
    if cancelled() { return None; }

    Some(RgbData {
        data: output,
        width,
        height,
        interpolate,
        scale_factors,
    })
}

#[cfg(test)]
mod image_color_cancellation_tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn one_shot_cancel_stays_latched_across_optional_mask_fallback() {
        let checks = Cell::new(0);
        let signal = || { checks.set(checks.get() + 1); checks.get() == 2 };
        let cancelled = latch_cancellation(&signal);
        assert!(!cancelled());
        assert!(cancelled());
        for _ in 0..100 { assert!(cancelled()); }
        assert_eq!(checks.get(), 2);
    }

    #[test]
    fn nonlinear_pdf_transfer_table_matches_direct_function_for_every_byte() {
        use hayro_syntax::object::{Dict, FromBytes, Object};
        for declaration in [
            b"<< /FunctionType 2 /Domain [0 1] /Range [0 1] /C0 [0.1] /C1 [0.9] /N 2.2 >>".as_slice(),
            b"<< /FunctionType 2 /Domain [0 1] /Range [0 1] /C0 [1] /C1 [0] /N 0.5 >>".as_slice(),
            b"<< /FunctionType 2 /Domain [0.2 0.8] /Range [0.1 0.9] /C0 [-1] /C1 [2] /N 3 >>".as_slice(),
        ] {
            let function = Function::new(&Object::Dict(Dict::from_bytes(declaration).unwrap())).unwrap();
            let direct = |value: u8| function.eval(smallvec![value as f32 / 255.0])
                .and_then(|v| v.first().copied()).map(|v| (v * 255.0 + 0.5) as u8).unwrap_or(value);
            let mut table = [None; 256];
            let calls = Cell::new(0);
            for index in 0..6147 {
                let value = (index % 256) as u8;
                assert_eq!(transfer_lookup(&mut table, value, |value| {
                    calls.set(calls.get() + 1); direct(value)
                }), direct(value));
            }
            assert_eq!(calls.get(), 256);
        }
    }

    #[test]
    fn transfer_table_is_exact_and_evaluates_each_present_value_once() {
        for channel in 0..3u8 {
            let calls = Cell::new(0);
            let evaluate = |value: u8| value.wrapping_mul(73).wrapping_add(channel);
            let source: Vec<u8> = (0..6147).map(|i| (i % 256) as u8).collect();
            let mut table = [None; 256];
            let actual: Vec<_> = source.iter().map(|&value| transfer_lookup(&mut table, value, |value| {
                calls.set(calls.get() + 1); evaluate(value)
            })).collect();
            assert_eq!(actual, source.iter().copied().map(evaluate).collect::<Vec<_>>());
            assert_eq!(calls.get(), 256);
        }
        let mut table = [None; 256];
        assert_eq!(transfer_lookup(&mut table, 19, |v| v), 19);
        assert_eq!(transfer_lookup(&mut table, 19, |_| panic!("cached value reevaluated")), 19);
        assert_eq!(table.iter().filter(|v| v.is_some()).count(), 1);
    }

    #[test]
    fn mask_bytes_preserve_values_and_cancel_at_every_checkpoint() {
        let cs = ColorSpace::device_gray();
        for bits in [1u8, 8] {
            let source: Vec<u8> = (0..if bits == 1 { 256 * 3 } else { 2048 * 3 })
                .map(|i| (i % 256) as u8).collect();
            for inverted_decode in [false, true] {
                let decode = if inverted_decode { cs.inverted_default_decode_arr(bits as f32) }
                    else { cs.default_decode_arr(bits as f32) };
                for invert in [false, true] {
                    let expected: Vec<u8> = if bits == 8 { source.clone() } else {
                        source.iter().flat_map(|b| (0..8).map(move |bit| if b & (128 >> bit) == 0 { 0 } else { 255 })).collect()
                    }.into_iter().map(|v| if invert ^ inverted_decode { 255 - v } else { v }).collect();
                    let checks = Cell::new(0);
                    let mut height = 3;
                    let actual = decode_mask_bytes(Cow::Borrowed(&source), 2048, &mut height, &cs, bits, &decode, invert, &|| {
                        checks.set(checks.get() + 1); false
                    }).unwrap();
                    assert_eq!(actual, expected);
                    assert_eq!(height, 3);
                    for stop in 1..=checks.get() {
                        let count = Cell::new(0);
                        let mut height = 3;
                        assert!(decode_mask_bytes(Cow::Borrowed(&source), 2048, &mut height, &cs, bits, &decode, invert, &|| {
                            count.set(count.get() + 1); count.get() == stop
                        }).is_none());
                        assert_eq!(count.get(), stop);
                    }
                }
            }
        }
    }

    #[test]
    fn matte_conversion_checks_every_block_and_preserves_values() {
        for rgb in [false, true] {
            let channels = if rgb { 3 } else { 1 };
            let source: Vec<u8> = (0..6147 * channels).map(|i| (i % 256) as u8).collect();
            let alpha: Vec<u8> = (0..6147).map(|i| (i % 256) as u8).collect();
            let matte = [31u8, 73, 129];
            let make = || if rgb {
                ImageData::Rgb(RgbData { data: source.clone(), width: 2049, height: 3, interpolate: false, scale_factors: (1.0, 1.0) })
            } else {
                ImageData::Luma(LumaData { data: source.clone(), width: 2049, height: 3, interpolate: false, scale_factors: (1.0, 1.0) })
            };
            let mut expected = source.clone();
            for (pixel, &a) in expected.chunks_mut(channels).zip(&alpha) {
                if a != 0 {
                    for (c, &m) in pixel.iter_mut().zip(&matte) {
                        *c = (m as f32 + (*c as f32 - m as f32) * (255.0 / a as f32)) as u8;
                    }
                }
            }
            let mut image = make();
            let checks = Cell::new(0);
            unpremultiply(&mut image, &alpha, &matte, &|| { checks.set(checks.get() + 1); false }).unwrap();
            let actual = match image { ImageData::Rgb(v) => v.data, ImageData::Luma(v) => v.data };
            assert_eq!(actual, expected);
            assert!(checks.get() > 7);
            for stop in 1..=checks.get() {
                let count = Cell::new(0);
                assert!(unpremultiply(&mut make(), &alpha, &matte, &|| { count.set(count.get() + 1); count.get() == stop }).is_none());
                assert_eq!(count.get(), stop);
            }
        }
    }

    #[test]
    fn u8_block_conversion_is_exact_and_cancellation_returns_no_partial_buffer() {
        for cs in [ColorSpace::device_gray(), ColorSpace::device_rgb(), ColorSpace::device_cmyk()] {
            let (width, height) = (2049u32, 3u32);
            let components = cs.num_components() as usize;
            let source: Vec<_> = (0..width as usize * height as usize * components)
                .map(|i| (i % 256) as u8).collect();
            let mut expected = vec![0; width as usize * height as usize * 3];
            cs.convert_u8(&source, &mut expected).unwrap();
            let checkpoints = Cell::new(0);
            let actual = get_rgb_u8_data(&source, width, height, &cs, &|| {
                checkpoints.set(checkpoints.get() + 1);
                false
            }).unwrap();
            assert_eq!(actual, expected);
            let count = checkpoints.get();
            assert!(count > 7);
            for stop in 1..=count {
                let current = Cell::new(0);
                assert!(get_rgb_u8_data(&source, width, height, &cs, &|| {
                    current.set(current.get() + 1);
                    current.get() == stop
                }).is_none());
                assert_eq!(current.get(), stop);
            }
        }
    }

    #[test]
    fn block_conversion_preserves_pixels_and_refuses_partial_output() {
        for cs in [ColorSpace::device_gray(), ColorSpace::device_rgb(), ColorSpace::device_cmyk()] {
            let (width, height) = (2049u32, 3u32);
            let components = cs.num_components() as usize;
            let source: Vec<_> = (0..width as usize * height as usize * components)
                .map(|i| (i % 257) as f32 / 256.).collect();
            let mut expected = vec![0; width as usize * height as usize * 3];
            cs.convert_f32(&source, &mut expected, false);
            let checkpoints = Cell::new(0);
            let image = get_rgb_data(&source, width, height, (1., 1.), &cs, false, &|| {
                checkpoints.set(checkpoints.get() + 1);
                false
            }).unwrap();
            assert_eq!(image.data, expected);
            assert_eq!((image.width, image.height), (width, height));
            let count = checkpoints.get();
            assert!(count > 7);
            for stop in 1..=count {
                let current = Cell::new(0);
                assert!(get_rgb_data(&source, width, height, (1., 1.), &cs, false, &|| {
                    current.set(current.get() + 1);
                    current.get() == stop
                }).is_none());
                assert_eq!(current.get(), stop);
            }
        }
    }
}

impl CacheKey for ImageXObject<'_> {
    fn cache_key(&self) -> u128 {
        let defaults = self.defaults.cache_key();
        let space = self.color_space.as_ref().map_or(0, ColorSpace::resource_key);
        crate::util::hash128(&(self.stream.cache_key(), defaults, space))
    }
}

#[must_use]
fn fix_image_length<T: Copy>(
    image: &mut Vec<T>,
    width: u32,
    height: &mut u32,
    filler: T,
    cs: &ColorSpace,
) -> Option<()> {
    let row_len = width as usize * cs.num_components() as usize;

    if (row_len * *height as usize) <= image.len() {
        // Too much data (or just the right amount), truncate it.
        image.truncate(row_len * *height as usize);
    } else {
        // Too little data, adapt the height and pad.
        *height = image.len().div_ceil(row_len) as u32;

        if !image.len().is_multiple_of(row_len) {
            image.extend(iter::repeat_n(filler, row_len - (image.len() % row_len)));
        }
    }

    if width == 0 || *height == 0 {
        None
    } else {
        Some(())
    }
}

fn get_components(
    data: &[u8],
    width: u32,
    height: u32,
    color_space: &ColorSpace,
    bits_per_component: u8,
) -> Option<Vec<u16>> {
    let result = match bits_per_component {
        1..8 | 9..16 => {
            let mut buf = vec![];
            let bpc = bits_per_component;
            let mut reader = BitReader::new(data);

            for _ in 0..height {
                for _ in 0..width {
                    for _ in 0..color_space.num_components() {
                        // See `stream_ccit_not_enough_data`, some images seemingly don't have
                        // enough data, so we just pad with zeroes in this case.
                        let next = reader.read(bpc).unwrap_or(0) as u16;

                        buf.push(next);
                    }
                }

                reader.align();
            }

            buf
        }
        8 => data.iter().map(|v| *v as u16).collect(),
        16 => {
            if data.len()%2 != 0 { return None; }
            data.chunks_exact(2).map(|v| u16::from_be_bytes([v[0],v[1]])).collect()
        },
        _ => {
            warn!("unsupported bits per component: {bits_per_component}");
            return None;
        }
    };

    Some(result)
}

fn apply_decode_array(
    components: &[u16],
    color_space: &ColorSpace,
    bits_per_component: u8,
    decode: &[(f32, f32)],
) -> Option<Vec<f32>> {
    let interpolate = |n: f32, d_min: f32, d_max: f32| {
        interpolate(
            n,
            0.0,
            2.0_f32.powi(bits_per_component as i32) - 1.0,
            d_min,
            d_max,
        )
    };

    let mut decoded_arr = vec![];

    for pixel in components.chunks(color_space.num_components() as usize) {
        for (component, (d_min, d_max)) in pixel.iter().zip(decode) {
            decoded_arr.push(interpolate(*component as f32, *d_min, *d_max));
        }
    }

    Some(decoded_arr)
}

#[test]
fn odd_16bit_component_payload_refuses_without_panic() {
    assert!(get_components(&[0x12],1,1,&ColorSpace::device_gray(),16).is_none());
    assert_eq!(get_components(&[0x12,0x34],1,1,&ColorSpace::device_gray(),16).unwrap(),vec![0x1234]);
}
