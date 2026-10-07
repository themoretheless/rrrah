use crate::color::{Color, ColorComponents, ColorSpace};
use crate::context::{Context, InterpreterCache};
use crate::device::Device;
use crate::function::Function;
use crate::interpret::state::State;
use crate::util::hash128;
use crate::x_object::{FormXObject, draw_form_xobject};
use crate::{CacheKey, InterpreterSettings};
use hayro_syntax::object::Name;
use hayro_syntax::object::ObjectIdentifier;
use hayro_syntax::object::Stream;
use hayro_syntax::object::dict::keys::*;
use hayro_syntax::object::{Dict, Object};
use hayro_syntax::page::Resources;
use hayro_syntax::xref::XRef;
use kurbo::Affine;
use smallvec::smallvec;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;

/// Type type of mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskType {
    /// A luminosity mask.
    Luminosity,
    /// An alpha mask.
    Alpha,
}

/// A transfer function to apply to the opacity values of a mask.
pub struct TransferFunction(Function);

impl TransferFunction {
    /// Apply the transfer function to the given value.
    ///
    /// The input value needs to be between 0 and 1 and the return value is
    /// guaranteed to be between 0 and 1.
    #[inline]
    pub fn apply(&self, val: f32) -> f32 {
        self.0
            .eval(smallvec![val])
            .and_then(|v| v.first().copied())
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
    }
}

struct Repr<'a> {
    obj_id: ObjectIdentifier,
    definition_key: u128,
    resource_key: u128,
    device_luminosity: bool,
    group: FormXObject<'a>,
    mask_type: MaskType,
    parent_resources: Resources<'a>,
    root_transform: Affine,
    bbox: kurbo::Rect,
    interpreter_cache: InterpreterCache<'a>,
    transfer_function: Option<TransferFunction>,
    settings: InterpreterSettings,
    background: Color,
    xref: &'a XRef,
    nesting_depth: u32,
}

impl Hash for Repr<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.definition_key.hash(state);
        self.resource_key.hash(state);
        self.root_transform.cache_key().hash(state);
    }
}

// Resource lookup inherits through nested scopes; every dictionary in the
// chain can change the same group's rendered mask without changing its stream.
fn resource_cache_key(resources: &Resources<'_>) -> u128 {
    hash128(&(
        resources.ext_g_states.cache_key(),
        resources.fonts.cache_key(),
        resources.properties.cache_key(),
        resources.color_spaces.cache_key(),
        resources.x_objects.cache_key(),
        resources.patterns.cache_key(),
        resources.shadings.cache_key(),
        resources.parent().map(resource_cache_key),
    ))
}

/// A soft mask.
#[derive(Clone, Hash)]
pub struct SoftMask<'a>(Rc<Repr<'a>>);

impl Debug for SoftMask<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SoftMask({:?})", self.0.obj_id)
    }
}

impl PartialEq for SoftMask<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cache_key() == other.cache_key()
    }
}

impl Eq for SoftMask<'_> {}

impl CacheKey for SoftMask<'_> {
    fn cache_key(&self) -> u128 {
        hash128(self)
    }
}

impl<'a> SoftMask<'a> {
    pub(crate) fn new(
        dict: &Dict<'a>,
        context: &Context<'a>,
        parent_resources: Resources<'a>,
    ) -> Option<Self> {
        // The same group may have distinct mask kinds, backgrounds and transfers.
        let definition_key = dict.cache_key();
        let obj_id = dict.get_ref(G)?.into();
        let group_stream = dict.get::<Stream<'_>>(G)?;
        let group = FormXObject::new(&group_stream)?;
        let kind = match dict.get::<Name<'_>>(S)?.deref() {
            LUMINOSITY => MaskType::Luminosity,
            ALPHA => MaskType::Alpha,
            _ => return None,
        };
        let cs = group
            .dict
            .get::<Dict<'_>>(GROUP)?
            .get::<Object<'_>>(CS)
            .and_then(|object| ColorSpace::new(object, &context.interpreter_cache.object_cache));
        // Alpha derives solely from coverage; a group CS is mandatory only
        // for luminosity masks. Do not discard an alpha mask without one.
        let cs = match (cs, kind) {
            (Some(cs), _) => cs,
            (None, MaskType::Alpha) => ColorSpace::device_gray(),
            (None, MaskType::Luminosity) => return None,
        };
        let resources = Resources::from_parent(
            group.dict.get::<Dict<'_>>(RESOURCES).unwrap_or_default(),
            parent_resources.clone(),
        );
        let cs = cs.with_device_defaults(&context.device_color_defaults(&resources));
        let device_luminosity = cs.is_device();
        let transfer_function = dict
            .get::<Object<'_>>(TR)
            .and_then(|o| Function::new(&o))
            .map(TransferFunction);
        let (mask_type, background) = match kind {
            MaskType::Luminosity => {
                let color = dict
                    .get::<ColorComponents>(BC)
                    .map(|c| Color::new(cs, c, 1.0))
                    .unwrap_or(Color::new(ColorSpace::device_gray(), smallvec![0.0], 1.0));

                (MaskType::Luminosity, color)
            }
            MaskType::Alpha => (
                MaskType::Alpha,
                // Background color attribute should only be used with luminosity masks.
                Color::new(ColorSpace::device_gray(), smallvec![0.0], 1.0),
            ),
        };
        let nesting_depth = context.nesting_depth() + 1;

        Some(Self(Rc::new(Repr {
            obj_id,
            definition_key,
            resource_key: resource_cache_key(&resources),
            device_luminosity,
            group,
            mask_type,
            root_transform: context.get().ctm,
            transfer_function,
            bbox: context.bbox(),
            interpreter_cache: context.interpreter_cache.clone(),
            settings: context.settings.clone(),
            xref: context.xref,
            background,
            parent_resources,
            nesting_depth,
        })))
    }

    /// Interpret the contents of the mask into the given device.
    pub fn interpret(&self, device: &mut impl Device<'a>) {
        let state = State::new(self.0.root_transform);
        let mut ctx = Context::new_with(
            self.0.root_transform,
            self.0.bbox,
            &self.0.interpreter_cache,
            self.0.xref,
            self.0.settings.clone(),
            state,
            self.0.nesting_depth,
        );
        draw_form_xobject(&self.0.parent_resources, &self.0.group, &mut ctx, device);
    }

    /// Return the object identifier of the mask.
    ///
    /// This can be used as a unique identifier for caching purposes.
    pub fn id(&self) -> ObjectIdentifier {
        self.0.obj_id
    }

    /// Whether luminosity uses the PDF device-space RGB approximation.
    pub fn uses_device_luminosity(&self) -> bool {
        self.0.device_luminosity
    }

    /// Return the underlying mask type.
    pub fn mask_type(&self) -> MaskType {
        self.0.mask_type
    }

    /// The background color against which the mask should be composited.
    pub fn background_color(&self) -> Color {
        self.0.background.clone()
    }

    /// Return the transfer function that should be used for the mask.
    pub fn transfer_function(&self) -> Option<&TransferFunction> {
        self.0.transfer_function.as_ref()
    }
}
