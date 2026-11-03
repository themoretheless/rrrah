//! Bounded native scalar/procedure/array execution with admitted VM rollback.
//! Full PostScript objects remain unsupported; unknown tokens return an error.
use crate::{
    EpsDeviceColor, EpsGraphics, EpsGraphicsCheckpoint, EpsGraphicsError, EpsGraphicsLimits, EpsNumber,
    EpsPaintKind, EpsProgram, EpsToken, EpsVectorScene,
};
use rrrah_core::{BufferError, MemoryBudget, MutableBuffer, SharedBuffer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpsOperator {
    Dict,
    Begin,
    End,
    CurrentDict,
    Known,
    Save,
    Restore,
    Add,
    Sub,
    Mul,
    Div,
    IntegerDiv,
    Remainder,
    Sqrt,
    Ln,
    Log,
    ConvertInteger,
    ConvertReal,
    Sin,
    Cos,
    Atan,
    Exp,
    Neg,
    Abs,
    Ceiling,
    Floor,
    Round,
    Truncate,
    Dup,
    Exch,
    Pop,
    Clear,
    Count,
    Mark,
    ArrayEnd,
    Array,
    Put,
    ALoad,
    AStore,
    GetInterval,
    PutInterval,
    Length,
    Get,
    Index,
    Copy,
    Roll,
    Def,
    Load,
    Bind,
    Exec,
    If,
    IfElse,
    Repeat,
    For,
    Loop,
    Exit,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Not,
    And,
    Or,
    Xor,
    NewPath,
    MoveTo,
    RelativeMoveTo,
    LineTo,
    RelativeLineTo,
    CurveTo,
    RelativeCurveTo,
    Arc,
    ArcNegative,
    ClosePath,
    Clip,
    EvenOddClip,
    InitClip,
    Stroke,
    Fill,
    EvenOddFill,
    RectFill,
    RectStroke,
    RectClip,
    SetGray,
    SetRgbColor,
    SetCmykColor,
    SetLineWidth,
    SetLineCap,
    SetLineJoin,
    SetMiterLimit,
    GSave,
    GRestore,
    GRestoreAll,
    Translate,
    Scale,
    Rotate,
    InitMatrix,
    Concat,
    InitGraphics,
    CurrentPoint,
    PathBBox,
    CurrentLineWidth,
    CurrentLineCap,
    CurrentLineJoin,
    CurrentMiterLimit,
    CurrentGray,
    CurrentRgbColor,
    CurrentCmykColor,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EpsValue<'a> {
    Null,
    /// Evaluation-local opaque save identity; cannot be reused after restore.
    Save {
        identity: u64,
    },
    Mark,
    Dictionary {
        identity: usize,
    },
    Array {
        begin: usize,
        length: usize,
    },
    Number(EpsNumber),
    Boolean(bool),
    LiteralName(&'a [u8]),
    Procedure {
        begin: usize,
        end: usize,
        identity: usize,
    },
    Operator(EpsOperator),
}
#[derive(Debug, Clone, Copy)]
pub struct EpsVmLimits {
    pub max_operands: usize,
    /// Capacity of the initial user table and, separately, the shared dynamic
    /// dictionary record arena (including one identity record per dictionary).
    pub max_dictionary_entries: usize,
    /// Includes the top-level execution frame. Also bounds dictionary-stack
    /// depth including the initial user dictionary, and save depth (capped at 128).
    pub max_execution_frames: usize,
    /// Includes instructions, dictionary hashing/probes, and bulk stack work.
    pub max_work: usize,
}
impl Default for EpsVmLimits {
    fn default() -> Self {
        Self {
            max_operands: 4096,
            max_dictionary_entries: 4096,
            max_execution_frames: 128,
            max_work: 1_000_000,
        }
    }
}
#[derive(Debug, Clone)]
pub struct EpsEvaluation<'a> {
    operands: SharedBuffer<EpsValue<'a>>,
    length: usize,
    // Array handles in operands borrow this retained arena.
    _arrays: SharedBuffer<EpsValue<'a>>,
    _dict_records: SharedBuffer<DictRecord<'a>>,
    _user_dictionary: Option<SharedBuffer<Entry<'a>>>,
}
impl<'a> EpsEvaluation<'a> {
    pub fn values(&self) -> &[EpsValue<'a>] {
        &self.operands[..self.length]
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpsVmLimit {
    Operands,
    ArrayItems,
    Dictionary,
    ExecutionFrames,
    Work,
    Saves,
    DictionaryStack,
}
#[derive(Debug, thiserror::Error)]
pub enum EpsVmError {
    #[error("PostScript execution cancelled")]
    Cancelled,
    #[error("PostScript execution limit: {0:?}")]
    Limit(EpsVmLimit),
    #[error("PostScript unmatched array mark")]
    UnmatchedMark,
    #[error("PostScript exit outside a loop")]
    InvalidExit,
    #[error("PostScript restore would invalidate a live object or save identity")]
    InvalidRestore,
    #[error("PostScript dictionary stack underflow")]
    DictionaryStackUnderflow,
    #[error("PostScript operand stack underflow")]
    StackUnderflow,
    #[error("PostScript operand type mismatch")]
    Type,
    #[error("PostScript numeric or operand range error")]
    Range,
    #[error("PostScript undefined numeric result")]
    UndefinedResult,
    #[error("undefined PostScript name at instruction {0}")]
    UndefinedName(usize),
    #[error("unsupported PostScript object at instruction {0}")]
    Unsupported(usize),
    #[error(transparent)]
    Memory(#[from] BufferError),
    #[error(transparent)]
    Graphics(#[from] EpsGraphicsError),
}
struct Work<F> {
    remaining: usize,
    cancelled: F,
}
impl<F: FnMut() -> bool> Work<F> {
    fn tick(&mut self) -> Result<(), EpsVmError> {
        if (self.cancelled)() {
            return Err(EpsVmError::Cancelled);
        }
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(EpsVmError::Limit(EpsVmLimit::Work))?;
        Ok(())
    }
}
#[derive(Debug, Clone, Copy)]
struct Entry<'a> {
    key: Option<&'a [u8]>,
    value: EpsValue<'a>,
}
#[derive(Debug, Clone, Copy)]
struct ForLoop {
    current: EpsNumber,
    increment: EpsNumber,
    limit: EpsNumber,
}
impl ForLoop {
    fn contains(self, value: EpsNumber) -> bool {
        self.contains_double(as_double(value))
    }
    fn contains_double(self, value: f64) -> bool {
        if as_double(self.increment) >= 0. {
            value <= as_double(self.limit)
        } else {
            value >= as_double(self.limit)
        }
    }
    fn next(self) -> Result<Option<EpsNumber>, EpsVmError> {
        if let (EpsNumber::Integer(a), EpsNumber::Integer(b)) = (self.current, self.increment) {
            if a.checked_add(b).is_none() {
                // Test the exact next integer before f32 promotion can round it
                // back onto i32::MIN and repeat an already completed endpoint.
                let next = f64::from(a) + f64::from(b);
                return if self.contains_double(next) {
                    Err(EpsVmError::UndefinedResult)
                } else {
                    Ok(None)
                };
            }
        }
        let next = match arithmetic(EpsOperator::Add, self.current, self.increment) {
            Ok(next) => next,
            Err(error) => {
                if !self.contains_double(as_double(self.current) + as_double(self.increment)) {
                    return Ok(None);
                }
                return Err(error);
            }
        };
        Ok(self.contains(next).then_some(next))
    }
}
#[derive(Debug, Clone, Copy)]
enum Continuation {
    Once,
    Repeat(usize),
    For(ForLoop),
    Loop,
}
#[derive(Debug, Clone, Copy)]
struct Frame {
    begin: usize,
    pc: usize,
    end: usize,
    continuation: Continuation,
}
#[derive(Clone, Copy, Default)]
struct Binding {
    operator: Option<EpsOperator>,
    // bind makes subsidiary executable arrays read-only, not the outer array.
    readonly: bool,
}
#[derive(Debug, Clone, Copy)]
struct DictRecord<'a> {
    owner: usize,
    entry: Entry<'a>,
}
impl Default for DictRecord<'_> {
    fn default() -> Self {
        Self {
            owner: 0,
            entry: Entry {
                key: None,
                value: EpsValue::Null,
            },
        }
    }
}
struct VmSave<'a> {
    identity: u64,
    dictionary: MutableBuffer<Entry<'a>>,
    dict_records: MutableBuffer<DictRecord<'a>>,
    dict_used: usize,
    arrays: MutableBuffer<EpsValue<'a>>,
    array_used: usize,
    bindings: MutableBuffer<Binding>,
    procedures: MutableBuffer<bool>,
    graphics: Option<EpsGraphicsCheckpoint>,
    previous: Option<Box<VmSave<'a>>>,
    _credit: rrrah_core::Reservation,
}
fn snapshot_buffer<T: Copy, F: FnMut() -> bool>(
    source: &[T],
    fill: T,
    budget: &MemoryBudget,
    work: &mut Work<F>,
) -> Result<MutableBuffer<T>, EpsVmError> {
    work.tick()?;
    let mut destination = budget.try_buffer(source.len(), fill)?;
    for (to, from) in destination.iter_mut().zip(source) {
        work.tick()?;
        *to = *from;
    }
    Ok(destination)
}
struct Machine<'a, 'g> {
    tape: &'g [crate::EpsInstruction<'a>],
    saves: Option<Box<VmSave<'a>>>,
    save_depth: usize,
    next_save: u64,
    dict_stack: MutableBuffer<EpsValue<'a>>,
    dict_depth: usize,
    retain_user_dictionary: bool,
    bindings: MutableBuffer<Binding>,
    graphics: Option<&'g mut EpsGraphics>,
    operands: MutableBuffer<EpsValue<'a>>,
    arrays: MutableBuffer<EpsValue<'a>>,
    array_used: usize,
    array_capacity: usize,
    array_budget: MemoryBudget,
    length: usize,
    dictionary: MutableBuffer<Entry<'a>>,
    dict_records: MutableBuffer<DictRecord<'a>>,
    dict_used: usize,
    frames: MutableBuffer<Frame>,
    depth: usize,
}
impl<'a> Machine<'a, '_> {
    fn current_dictionary(&self) -> usize {
        match self.dict_stack.get(self.dict_depth.wrapping_sub(1)) {
            Some(EpsValue::Dictionary { identity }) => *identity,
            _ => 0,
        }
    }
    fn dict_find<F: FnMut() -> bool>(
        &self,
        identity: usize,
        key: &[u8],
        work: &mut Work<F>,
    ) -> Result<Option<usize>, EpsVmError> {
        if identity == 0 {
            return Ok(self
                .slot(key, work)?
                .filter(|index| self.dictionary[*index].key.is_some()));
        }
        for index in (0..self.dict_used).rev() {
            work.tick()?;
            let record = self.dict_records[index];
            if record.owner != identity {
                continue;
            }
            if let Some(existing) = record.entry.key {
                if existing.len() != key.len() {
                    continue;
                }
                let mut equal = true;
                for (&a, &b) in existing.iter().zip(key) {
                    work.tick()?;
                    if a != b {
                        equal = false;
                        break;
                    }
                }
                if equal {
                    return Ok(Some(index));
                }
            }
        }
        Ok(None)
    }
    fn dict_value(&self, identity: usize, index: usize) -> EpsValue<'a> {
        if identity == 0 {
            self.dictionary[index].value
        } else {
            self.dict_records[index].entry.value
        }
    }
    fn dict_put<F: FnMut() -> bool>(
        &mut self,
        identity: usize,
        key: &'a [u8],
        value: EpsValue<'a>,
        work: &mut Work<F>,
    ) -> Result<(), EpsVmError> {
        let index = if identity == 0 {
            self.slot(key, work)?
                .ok_or(EpsVmError::Limit(EpsVmLimit::Dictionary))?
        } else if let Some(index) = self.dict_find(identity, key, work)? {
            index
        } else {
            if self.dict_used == self.dict_records.len() {
                return Err(EpsVmError::Limit(EpsVmLimit::Dictionary));
            }
            work.tick()?;
            let index = self.dict_used;
            self.dict_used += 1;
            index
        };
        let entry = Entry {
            key: Some(key),
            value,
        };
        if identity == 0 {
            self.dictionary[index] = entry;
        } else {
            self.dict_records[index] = DictRecord {
                owner: identity,
                entry,
            };
        }
        Ok(())
    }
    fn dictionary_identity(&mut self) -> Result<usize, EpsVmError> {
        match self.pop()? {
            EpsValue::Dictionary { identity } => Ok(identity),
            _ => Err(EpsVmError::Type),
        }
    }
    fn save_vm<F: FnMut() -> bool>(&mut self, work: &mut Work<F>) -> Result<(), EpsVmError> {
        if self.length == self.operands.len() {
            return Err(EpsVmError::Limit(EpsVmLimit::Operands));
        }
        // Bound snapshot nesting by the caller frame limit and a hard 128-save cap.
        if self.save_depth == self.frames.len().min(128) {
            return Err(EpsVmError::Limit(EpsVmLimit::Saves));
        }
        let next = self
            .next_save
            .checked_add(1)
            .ok_or(EpsVmError::Limit(EpsVmLimit::Saves))?;
        let credit = self
            .array_budget
            .try_reserve(std::mem::size_of::<VmSave<'a>>() as u64)?;
        let dictionary = snapshot_buffer(
            &self.dictionary,
            Entry {
                key: None,
                value: EpsValue::Null,
            },
            &self.array_budget,
            work,
        )?;
        let dict_records = snapshot_buffer(
            &self.dict_records,
            DictRecord::default(),
            &self.array_budget,
            work,
        )?;
        let arrays = snapshot_buffer(&self.arrays, EpsValue::Null, &self.array_budget, work)?;
        let bindings = snapshot_buffer(&self.bindings, Binding::default(), &self.array_budget, work)?;
        work.tick()?;
        let mut procedures = self.array_budget.try_buffer(self.tape.len(), false)?;
        // Source procedures remain diagnostic instruction-range objects in this
        // VM. Capture all reachable pre-save ranges, including nested literals.
        let mut mark = |range: Option<(usize, usize)>| -> Result<(), EpsVmError> {
            work.tick()?;
            let Some((begin, end)) = range else {
                return Ok(());
            };
            if begin == 0 && end == 0 {
                return Ok(());
            } // importer showpage
            let opening = begin.checked_sub(1).ok_or(EpsVmError::InvalidRestore)?;
            for at in opening..end {
                work.tick()?;
                if matches!(self.tape[at].token, EpsToken::ProcedureStart) {
                    procedures[at] = true;
                }
            }
            Ok(())
        };
        for value in self.operands[..self.length]
            .iter()
            .chain(self.arrays[..self.array_used].iter())
            .chain(self.dictionary.iter().map(|entry| &entry.value))
            .chain(
                self.dict_records[..self.dict_used]
                    .iter()
                    .map(|record| &record.entry.value),
            )
        {
            mark(if let EpsValue::Procedure { begin, end, .. } = *value {
                Some((begin, end))
            } else {
                None
            })?;
        }
        for frame in &self.frames[1..self.depth] {
            mark(Some((frame.begin, frame.end)))?;
        }
        let graphics = if let Some(graphics) = self.graphics.as_ref() {
            let mut failure = None;
            let result = graphics.checkpoint(&self.array_budget, || match work.tick() {
                Ok(()) => false,
                Err(error) => {
                    failure = Some(error);
                    true
                }
            });
            if let Some(error) = failure {
                return Err(error);
            }
            Some(result?)
        } else {
            None
        };
        work.tick()?;
        // No fallible work remains after the graphics boundary is published.
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.push_vm_save_boundary()?;
        }
        let save = Box::new(VmSave {
            identity: self.next_save,
            dictionary,
            dict_records,
            dict_used: self.dict_used,
            arrays,
            array_used: self.array_used,
            bindings,
            procedures,
            graphics,
            previous: self.saves.take(),
            _credit: credit,
        });
        self.operands[self.length] = EpsValue::Save {
            identity: self.next_save,
        };
        self.length += 1;
        self.next_save = next;
        self.save_depth += 1;
        self.saves = Some(save);
        Ok(())
    }
    fn restore_vm<F: FnMut() -> bool>(&mut self, work: &mut Work<F>) -> Result<(), EpsVmError> {
        let Some(value) = self.operands[..self.length].last().copied() else {
            return Err(EpsVmError::StackUnderflow);
        };
        let EpsValue::Save { identity } = value else {
            return Err(EpsVmError::Type);
        };
        let mut cursor = self.saves.as_deref();
        let mut removed = 0;
        let target = loop {
            work.tick()?;
            let save = cursor.ok_or(EpsVmError::InvalidRestore)?;
            removed += 1;
            if save.identity == identity {
                break save;
            }
            cursor = save.previous.as_deref();
        };
        let procedure_valid = |begin: usize, end: usize| {
            (begin == 0 && end == 0)
                || begin
                    .checked_sub(1)
                    .and_then(|at| target.procedures.get(at))
                    .copied()
                    .unwrap_or(false)
        };
        for value in &self.operands[..self.length - 1] {
            work.tick()?;
            let valid = match *value {
                EpsValue::Array { begin, length } => {
                    length == 0
                        || (begin > 0
                            && begin
                                .checked_add(length)
                                .is_some_and(|end| end <= target.array_used))
                }
                EpsValue::Procedure { begin, end, .. } => begin == end || procedure_valid(begin, end),
                EpsValue::Save { identity: other } => other <= identity,
                EpsValue::Dictionary { identity: id } => {
                    id == 0
                        || (id <= target.dict_used
                            && target.dict_records[id - 1].owner == id
                            && target.dict_records[id - 1].entry.key.is_none())
                }
                _ => true,
            };
            if !valid {
                return Err(EpsVmError::InvalidRestore);
            }
        }
        for value in &self.dict_stack[..self.dict_depth] {
            work.tick()?;
            let EpsValue::Dictionary { identity: id } = *value else {
                return Err(EpsVmError::InvalidRestore);
            };
            if id != 0
                && !(id <= target.dict_used
                    && target.dict_records[id - 1].owner == id
                    && target.dict_records[id - 1].entry.key.is_none())
            {
                return Err(EpsVmError::InvalidRestore);
            }
        }
        for frame in &self.frames[1..self.depth] {
            work.tick()?;
            // A finished one-shot procedure has no remaining execution reference;
            // this is the same tail-execution behavior as the reference interpreter.
            if !(frame.pc == frame.end && matches!(frame.continuation, Continuation::Once))
                && !procedure_valid(frame.begin, frame.end)
            {
                return Err(EpsVmError::InvalidRestore);
            }
        }
        work.tick()?;
        if let (Some(graphics), Some(checkpoint)) = (self.graphics.as_mut(), target.graphics.as_ref()) {
            let mut failure = None;
            let result = graphics.restore_checkpoint(checkpoint, || match work.tick() {
                Ok(()) => false,
                Err(error) => {
                    failure = Some(error);
                    true
                }
            });
            if let Some(error) = failure {
                return Err(error);
            }
            result?;
        }
        // All validation/cancellation precedes publication. Move admitted buffers
        // instead of allocating during commit; preserve operand/execution stacks.
        let mut save = self.saves.take().unwrap();
        while save.identity != identity {
            save = save.previous.take().unwrap();
        }
        self.saves = save.previous.take();
        self.dictionary = save.dictionary;
        self.dict_records = save.dict_records;
        self.dict_used = save.dict_used;
        self.arrays = save.arrays;
        self.array_used = save.array_used;
        self.bindings = save.bindings;
        self.length -= 1;
        self.save_depth -= removed;
        Ok(())
    }
    fn push(&mut self, value: EpsValue<'a>) -> Result<(), EpsVmError> {
        if self.length == self.operands.len() {
            return Err(EpsVmError::Limit(EpsVmLimit::Operands));
        }
        self.operands[self.length] = value;
        self.length += 1;
        Ok(())
    }
    fn pop(&mut self) -> Result<EpsValue<'a>, EpsVmError> {
        self.length = self.length.checked_sub(1).ok_or(EpsVmError::StackUnderflow)?;
        Ok(self.operands[self.length])
    }
    fn number(&mut self) -> Result<EpsNumber, EpsVmError> {
        match self.pop()? {
            EpsValue::Number(n) => Ok(n),
            _ => Err(EpsVmError::Type),
        }
    }
    fn integer(&mut self) -> Result<i32, EpsVmError> {
        match self.number()? {
            EpsNumber::Integer(n) => Ok(n),
            _ => Err(EpsVmError::Type),
        }
    }
    fn array_range<F: FnMut() -> bool>(
        &mut self,
        length: usize,
        work: &mut Work<F>,
    ) -> Result<(usize, usize), EpsVmError> {
        let end = self
            .array_used
            .checked_add(length)
            .and_then(|n| n.checked_add(1))
            .filter(|n| *n <= self.array_capacity)
            .ok_or(EpsVmError::Limit(EpsVmLimit::ArrayItems))?;
        if self.arrays.is_empty() {
            work.tick()?;
            self.arrays = self
                .array_budget
                .try_buffer(self.array_capacity, EpsValue::Null)?;
            work.tick()?;
        }
        Ok((self.array_used + 1, end))
    }
    fn array_handle(&mut self) -> Result<(usize, usize), EpsVmError> {
        let EpsValue::Array { begin, length } = self.pop()? else {
            return Err(EpsVmError::Type);
        };
        Ok((begin, length))
    }
    fn copy_array_items<F: FnMut() -> bool>(
        &mut self,
        source: usize,
        destination: usize,
        length: usize,
        work: &mut Work<F>,
    ) -> Result<(), EpsVmError> {
        let backward = destination > source && destination < source + length;
        for offset in 0..length {
            work.tick()?;
            let index = if backward { length - 1 - offset } else { offset };
            self.arrays[destination + index] = self.arrays[source + index];
        }
        Ok(())
    }
    fn boolean(&mut self) -> Result<bool, EpsVmError> {
        match self.pop()? {
            EpsValue::Boolean(n) => Ok(n),
            _ => Err(EpsVmError::Type),
        }
    }
    fn procedure(&mut self) -> Result<EpsValue<'a>, EpsVmError> {
        match self.pop()? {
            p @ EpsValue::Procedure { .. } => Ok(p),
            _ => Err(EpsVmError::Type),
        }
    }
    fn call(&mut self, value: EpsValue<'a>, remaining: usize) -> Result<(), EpsVmError> {
        let EpsValue::Procedure { begin, end, .. } = value else {
            return Err(EpsVmError::Type);
        };
        if remaining == 0 {
            return Ok(());
        }
        if self.depth == self.frames.len() {
            return Err(EpsVmError::Limit(EpsVmLimit::ExecutionFrames));
        }
        self.frames[self.depth] = Frame {
            begin,
            pc: begin,
            end,
            continuation: Continuation::Once,
        };
        self.depth += 1;
        Ok(())
    }
    fn slot<F: FnMut() -> bool>(&self, key: &[u8], work: &mut Work<F>) -> Result<Option<usize>, EpsVmError> {
        if self.dictionary.is_empty() {
            return Ok(None);
        }
        let mut hash = 0xcbf29ce484222325u64;
        for &b in key {
            work.tick()?;
            hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
        let mut index = (hash % self.dictionary.len() as u64) as usize;
        for _ in 0..self.dictionary.len() {
            work.tick()?;
            match self.dictionary[index].key {
                None => return Ok(Some(index)),
                Some(existing) => {
                    // Charge every compared name byte, including collision probes.
                    if existing.len() == key.len() {
                        let mut equal = true;
                        for (&a, &b) in existing.iter().zip(key) {
                            work.tick()?;
                            if a != b {
                                equal = false;
                                break;
                            }
                        }
                        if equal {
                            return Ok(Some(index));
                        }
                    }
                }
            }
            index = if index + 1 == self.dictionary.len() {
                0
            } else {
                index + 1
            };
        }
        Ok(None)
    }
    fn lookup<F: FnMut() -> bool>(
        &self,
        key: &[u8],
        at: usize,
        work: &mut Work<F>,
    ) -> Result<EpsValue<'a>, EpsVmError> {
        for handle in self.dict_stack[..self.dict_depth].iter().rev() {
            work.tick()?;
            if let EpsValue::Dictionary { identity } = *handle {
                if let Some(index) = self.dict_find(identity, key, work)? {
                    return Ok(self.dict_value(identity, index));
                }
            }
        }
        if let Some(index) = self.dict_find(0, key, work)? {
            return Ok(self.dictionary[index].value);
        }
        match key {
            b"true" => Ok(EpsValue::Boolean(true)),
            b"false" => Ok(EpsValue::Boolean(false)),
            b"null" => Ok(EpsValue::Null),
            // EPSF 3.0 section 3.2: the importer defines showpage as an
            // empty procedure. User definitions still win above.
            b"showpage" if self.graphics.is_some() => Ok(EpsValue::Procedure {
                begin: 0,
                end: 0,
                identity: 0,
            }),
            _ => operator(key)
                .map(EpsValue::Operator)
                .ok_or(EpsVmError::UndefinedName(at)),
        }
    }
    fn bind_procedure<F: FnMut() -> bool>(
        &mut self,
        begin: usize,
        end: usize,
        nested: bool,
        work: &mut Work<F>,
    ) -> Result<(), EpsVmError> {
        if begin == end {
            return Ok(());
        }
        let opening = begin.checked_sub(1).ok_or(EpsVmError::Type)?;
        if self.tape.get(opening).and_then(|i| i.procedure_partner) != Some(end) {
            return Err(EpsVmError::Type);
        }
        if self.bindings.is_empty() {
            work.tick()?;
            self.bindings = self
                .array_budget
                .try_buffer(self.tape.len(), Binding::default())?;
        }
        if !self.bindings[opening].readonly {
            let mut at = begin;
            while at < end {
                work.tick()?;
                let instruction = self.tape[at];
                match instruction.token {
                    EpsToken::ProcedureStart => {
                        let last = instruction.procedure_partner.ok_or(EpsVmError::Unsupported(at))?;
                        if self.bindings[at].readonly {
                            at = last + 1;
                            continue;
                        }
                        self.bindings[at].readonly = true;
                    }
                    EpsToken::Word(word)
                        if instruction.number.is_none() && self.bindings[at].operator.is_none() =>
                    {
                        match self.lookup(word, at, work) {
                            Ok(EpsValue::Operator(operator)) => self.bindings[at].operator = Some(operator),
                            Ok(_) | Err(EpsVmError::UndefinedName(_)) => {}
                            Err(error) => return Err(error),
                        }
                    }
                    _ => {}
                }
                at += 1;
            }
        }
        if nested {
            self.bindings[opening].readonly = true;
        }
        Ok(())
    }
    fn bind<F: FnMut() -> bool>(&mut self, work: &mut Work<F>) -> Result<(), EpsVmError> {
        let value = self.pop()?;
        match value {
            EpsValue::Procedure { begin, end, .. } => self.bind_procedure(begin, end, false, work)?,
            EpsValue::Array { begin, length } => {
                for index in begin..begin.checked_add(length).ok_or(EpsVmError::Range)? {
                    work.tick()?;
                    if let EpsValue::Procedure { begin, end, .. } = self.arrays[index] {
                        self.bind_procedure(begin, end, true, work)?;
                    }
                }
            }
            _ => return Err(EpsVmError::Type),
        }
        self.push(value)
    }
    fn operate<F: FnMut() -> bool>(
        &mut self,
        op: EpsOperator,
        at: usize,
        work: &mut Work<F>,
    ) -> Result<(), EpsVmError> {
        use EpsOperator as O;
        use EpsVmError as E;
        match op {
            O::Dict => {
                let hint = usize::try_from(self.integer()?).map_err(|_| E::Range)?;
                if hint > self.dictionary.len() || self.dict_used == self.dictionary.len() {
                    return Err(E::Limit(EpsVmLimit::Dictionary));
                }
                work.tick()?;
                if self.dict_records.is_empty() {
                    self.dict_records = self
                        .array_budget
                        .try_buffer(self.dictionary.len(), DictRecord::default())?;
                }
                work.tick()?;
                let identity = self.dict_used + 1;
                self.dict_records[self.dict_used] = DictRecord {
                    owner: identity,
                    entry: Entry {
                        key: None,
                        value: EpsValue::Null,
                    },
                };
                self.dict_used += 1;
                self.push(EpsValue::Dictionary { identity })?;
            }
            O::Begin => {
                let identity = self.dictionary_identity()?;
                if self.dict_depth + 1 >= self.frames.len() {
                    return Err(E::Limit(EpsVmLimit::DictionaryStack));
                }
                work.tick()?;
                if self.dict_stack.is_empty() {
                    self.dict_stack = self
                        .array_budget
                        .try_buffer(self.frames.len() - 1, EpsValue::Null)?;
                }
                work.tick()?;
                self.dict_stack[self.dict_depth] = EpsValue::Dictionary { identity };
                self.dict_depth += 1;
            }
            O::End => {
                self.dict_depth = self
                    .dict_depth
                    .checked_sub(1)
                    .ok_or(E::DictionaryStackUnderflow)?;
            }
            O::CurrentDict => {
                let identity = self.current_dictionary();
                if identity == 0 {
                    self.retain_user_dictionary = true;
                }
                self.push(EpsValue::Dictionary { identity })?;
            }
            O::Known => {
                let EpsValue::LiteralName(key) = self.pop()? else {
                    return Err(E::Type);
                };
                let identity = self.dictionary_identity()?;
                let known = self.dict_find(identity, key, work)?.is_some();
                self.push(EpsValue::Boolean(known))?;
            }
            O::Save => return self.save_vm(work),
            O::Restore => return self.restore_vm(work),
            O::Bind => return self.bind(work),
            O::NewPath
            | O::MoveTo
            | O::RelativeMoveTo
            | O::LineTo
            | O::RelativeLineTo
            | O::CurveTo
            | O::RelativeCurveTo
            | O::Arc
            | O::ArcNegative
            | O::ClosePath
            | O::Clip
            | O::EvenOddClip
            | O::InitClip
            | O::Stroke
            | O::Fill
            | O::EvenOddFill
            | O::RectFill
            | O::RectStroke
            | O::RectClip
            | O::SetGray
            | O::SetRgbColor
            | O::SetCmykColor
            | O::SetLineWidth
            | O::SetLineCap
            | O::SetLineJoin
            | O::SetMiterLimit
            | O::GSave
            | O::GRestore
            | O::GRestoreAll
            | O::Translate
            | O::Scale
            | O::Rotate
            | O::InitMatrix
            | O::Concat
            | O::InitGraphics
            | O::CurrentPoint
            | O::PathBBox
            | O::CurrentLineWidth
            | O::CurrentLineCap
            | O::CurrentLineJoin
            | O::CurrentMiterLimit
            | O::CurrentGray
            | O::CurrentRgbColor
            | O::CurrentCmykColor => return self.graphics_operation(op, at, work),
            O::Add | O::Sub | O::Mul | O::Div => {
                let b = self.number()?;
                let a = self.number()?;
                self.push(EpsValue::Number(arithmetic(op, a, b)?))?;
            }
            O::IntegerDiv | O::Remainder => {
                let b = i64::from(self.integer()?);
                let a = i64::from(self.integer()?);
                if b == 0 {
                    return Err(E::UndefinedResult);
                }
                let wide = if op == O::IntegerDiv { a / b } else { a % b };
                let value = i32::try_from(wide).map_err(|_| E::UndefinedResult)?;
                self.push(EpsValue::Number(EpsNumber::Integer(value)))?;
            }
            O::Sin | O::Cos => {
                // Signed reduction preserves tiny negative angles that would be
                // lost when adding a full turn to normalize into 0..360.
                let angle = as_double(self.number()?) % 360.;
                let value = match (op, angle) {
                    (O::Sin, 0. | 180. | -180.) | (O::Cos, 90. | -90. | 270. | -270.) => 0.,
                    (O::Sin, 90. | -270.) | (O::Cos, 0.) => 1.,
                    (O::Sin, 270. | -90.) | (O::Cos, 180. | -180.) => -1.,
                    (O::Sin, _) => angle.to_radians().sin(),
                    _ => angle.to_radians().cos(),
                };
                self.push(EpsValue::Number(EpsNumber::Real(value as f32)))?;
            }
            O::Atan => {
                let denominator = as_double(self.number()?);
                let numerator = as_double(self.number()?);
                if numerator == 0. && denominator == 0. {
                    return Err(E::UndefinedResult);
                }
                let mut angle = numerator.atan2(denominator).to_degrees();
                if angle < 0. {
                    angle += 360.;
                }
                // PostScript angles have no signed-zero distinction.
                if angle == 0. {
                    angle = 0.;
                }
                self.push(EpsValue::Number(EpsNumber::Real(angle as f32)))?;
            }
            O::Exp => {
                let exponent = as_double(self.number()?);
                let base = as_double(self.number()?);
                let result = base.powf(exponent) as f32;
                if !result.is_finite() {
                    return Err(E::UndefinedResult);
                }
                self.push(EpsValue::Number(EpsNumber::Real(result)))?;
            }
            O::Sqrt => {
                let value = as_double(self.number()?);
                if value < 0. {
                    return Err(E::Range);
                }
                self.push(EpsValue::Number(EpsNumber::Real(value.sqrt() as f32)))?;
            }
            O::Ln | O::Log => {
                let value = as_double(self.number()?);
                if value <= 0. {
                    return Err(E::Range);
                }
                let result = if op == O::Ln { value.ln() } else { value.log10() };
                self.push(EpsValue::Number(EpsNumber::Real(result as f32)))?;
            }
            O::ConvertInteger | O::ConvertReal => {
                let number = self.number()?;
                let result = if op == O::ConvertReal {
                    EpsNumber::Real(as_double(number) as f32)
                } else {
                    match number {
                        EpsNumber::Integer(_) => number,
                        EpsNumber::Real(value) => {
                            let truncated = f64::from(value).trunc();
                            if truncated < f64::from(i32::MIN) || truncated > f64::from(i32::MAX) {
                                return Err(E::Range);
                            }
                            EpsNumber::Integer(truncated as i32)
                        }
                    }
                };
                self.push(EpsValue::Number(result))?;
            }
            O::Neg | O::Abs => {
                let a = self.number()?;
                let value = match a {
                    EpsNumber::Integer(n) => {
                        let checked = if op == O::Neg {
                            n.checked_neg()
                        } else {
                            n.checked_abs()
                        };
                        checked
                            .map(EpsNumber::Integer)
                            .unwrap_or(EpsNumber::Real(2147483648.0))
                    }
                    EpsNumber::Real(n) => EpsNumber::Real(if op == O::Neg { -n } else { n.abs() }),
                };
                self.push(EpsValue::Number(value))?;
            }
            O::Ceiling | O::Floor | O::Round | O::Truncate => {
                let value = match self.number()? {
                    EpsNumber::Integer(n) => EpsNumber::Integer(n),
                    EpsNumber::Real(n) => EpsNumber::Real(match op {
                        O::Ceiling => n.ceil(),
                        O::Floor => n.floor(),
                        // PostScript ties go toward positive infinity. Compute
                        // in f64 so adding 0.5 cannot round a nearby f32 first.
                        O::Round => (f64::from(n) + 0.5).floor() as f32,
                        O::Truncate => n.trunc(),
                        _ => unreachable!(),
                    }),
                };
                self.push(EpsValue::Number(value))?;
            }
            O::Dup => {
                let value = *self
                    .operands
                    .get(self.length.checked_sub(1).ok_or(E::StackUnderflow)?)
                    .ok_or(E::StackUnderflow)?;
                self.push(value)?;
            }
            O::Exch => {
                if self.length < 2 {
                    return Err(E::StackUnderflow);
                };
                self.operands.swap(self.length - 1, self.length - 2);
            }
            O::Pop => {
                self.pop()?;
            }
            O::Clear => self.length = 0,
            O::Count => self.push(EpsValue::Number(EpsNumber::Integer(
                i32::try_from(self.length).map_err(|_| E::Range)?,
            )))?,
            O::Index => {
                let n = usize::try_from(self.integer()?).map_err(|_| E::Range)?;
                let index = self
                    .length
                    .checked_sub(n.checked_add(1).ok_or(E::Range)?)
                    .ok_or(E::StackUnderflow)?;
                self.push(self.operands[index])?;
            }
            O::Copy => match self.pop()? {
                EpsValue::Array {
                    begin: destination,
                    length: capacity,
                } => {
                    let (source, length) = self.array_handle()?;
                    if length > capacity {
                        return Err(E::Range);
                    }
                    self.copy_array_items(source, destination, length, work)?;
                    self.push(EpsValue::Array {
                        begin: destination,
                        length,
                    })?;
                }
                EpsValue::Number(EpsNumber::Integer(n)) => {
                    let n = usize::try_from(n).map_err(|_| E::Range)?;
                    let start = self.length.checked_sub(n).ok_or(E::StackUnderflow)?;
                    if self
                        .length
                        .checked_add(n)
                        .is_none_or(|end| end > self.operands.len())
                    {
                        return Err(E::Limit(EpsVmLimit::Operands));
                    }
                    for index in start..start + n {
                        work.tick()?;
                        self.push(self.operands[index])?;
                    }
                }
                _ => return Err(E::Type),
            },
            O::Roll => {
                let j = self.integer()?;
                let n = usize::try_from(self.integer()?).map_err(|_| E::Range)?;
                let start = self.length.checked_sub(n).ok_or(E::StackUnderflow)?;
                if n != 0 {
                    let shift = i64::from(j).rem_euclid(n as i64) as usize;
                    for (a, b) in [
                        (start, self.length),
                        (start, start + shift),
                        (start + shift, self.length),
                    ] {
                        for k in 0..(b - a) / 2 {
                            work.tick()?;
                            self.operands.swap(a + k, b - 1 - k);
                        }
                    }
                }
            }
            O::Def => {
                let value = self.pop()?;
                let EpsValue::LiteralName(key) = self.pop()? else {
                    return Err(E::Type);
                };
                self.dict_put(self.current_dictionary(), key, value, work)?;
            }
            O::Load => {
                let EpsValue::LiteralName(key) = self.pop()? else {
                    return Err(E::Type);
                };
                let value = self.lookup(key, at, work)?;
                self.push(value)?;
            }
            O::Exec => return Err(E::Unsupported(at)), // Tail-dispatched by execute.
            O::If => {
                let proc = self.procedure()?;
                if self.boolean()? {
                    self.call(proc, 1)?;
                }
            }
            O::IfElse => {
                let no = self.procedure()?;
                let yes = self.procedure()?;
                let condition = self.boolean()?;
                self.call(if condition { yes } else { no }, 1)?;
            }
            O::Mark => self.push(EpsValue::Mark)?,
            O::ArrayEnd => {
                let mut mark = None;
                for index in (0..self.length).rev() {
                    work.tick()?;
                    if self.operands[index] == EpsValue::Mark {
                        mark = Some(index);
                        break;
                    }
                }
                let mark = mark.ok_or(E::UnmatchedMark)?;
                let length = self.length - mark - 1;
                let (begin, end) = self.array_range(length, work)?;
                for index in 0..length {
                    work.tick()?;
                    self.arrays[begin + index] = self.operands[mark + 1 + index];
                }
                self.array_used = end;
                self.length = mark;
                self.push(EpsValue::Array { begin, length })?;
            }
            O::Array => {
                let length = usize::try_from(self.integer()?).map_err(|_| E::Range)?;
                let (begin, end) = self.array_range(length, work)?;
                for index in 0..length {
                    work.tick()?;
                    self.arrays[begin + index] = EpsValue::Null;
                }
                self.array_used = end;
                self.push(EpsValue::Array { begin, length })?;
            }
            O::Put => {
                let value = self.pop()?;
                let key = self.pop()?;
                let container = self.pop()?;
                match (container, key) {
                    (EpsValue::Dictionary { identity }, EpsValue::LiteralName(key)) => {
                        self.dict_put(identity, key, value, work)?
                    }
                    (EpsValue::Array { begin, length }, EpsValue::Number(EpsNumber::Integer(index))) => {
                        let index = usize::try_from(index).map_err(|_| E::Range)?;
                        if index >= length {
                            return Err(E::Range);
                        }
                        work.tick()?;
                        self.arrays[begin + index] = value;
                    }
                    _ => return Err(E::Type),
                }
            }
            O::ALoad => {
                let (begin, length) = self.array_handle()?;
                if self
                    .length
                    .checked_add(length)
                    .and_then(|n| n.checked_add(1))
                    .is_none_or(|n| n > self.operands.len())
                {
                    return Err(E::Limit(EpsVmLimit::Operands));
                }
                for index in 0..length {
                    work.tick()?;
                    self.push(self.arrays[begin + index])?;
                }
                self.push(EpsValue::Array { begin, length })?;
            }
            O::AStore => {
                let (begin, length) = self.array_handle()?;
                let start = self.length.checked_sub(length).ok_or(E::StackUnderflow)?;
                for index in 0..length {
                    work.tick()?;
                    self.arrays[begin + index] = self.operands[start + index];
                }
                self.length = start;
                self.push(EpsValue::Array { begin, length })?;
            }
            O::GetInterval => {
                let count = usize::try_from(self.integer()?).map_err(|_| E::Range)?;
                let index = usize::try_from(self.integer()?).map_err(|_| E::Range)?;
                let (begin, length) = self.array_handle()?;
                if index.checked_add(count).is_none_or(|end| end > length) {
                    return Err(E::Range);
                }
                self.push(EpsValue::Array {
                    begin: begin + index,
                    length: count,
                })?;
            }
            O::PutInterval => {
                let (source, count) = self.array_handle()?;
                let index = usize::try_from(self.integer()?).map_err(|_| E::Range)?;
                let (begin, length) = self.array_handle()?;
                if index.checked_add(count).is_none_or(|end| end > length) {
                    return Err(E::Range);
                }
                self.copy_array_items(source, begin + index, count, work)?;
            }
            O::Length => {
                let length = match self.pop()? {
                    EpsValue::Array { length, .. } => length,
                    EpsValue::Dictionary { identity } => {
                        let mut count = 0;
                        if identity == 0 {
                            for entry in self.dictionary.iter() {
                                work.tick()?;
                                count += usize::from(entry.key.is_some());
                            }
                        } else {
                            for record in &self.dict_records[..self.dict_used] {
                                work.tick()?;
                                count += usize::from(record.owner == identity && record.entry.key.is_some());
                            }
                        }
                        count
                    }
                    _ => return Err(E::Type),
                };
                self.push(EpsValue::Number(EpsNumber::Integer(
                    i32::try_from(length).map_err(|_| E::Range)?,
                )))?;
            }
            O::Get => {
                let key = self.pop()?;
                let value = match (self.pop()?, key) {
                    (EpsValue::Dictionary { identity }, EpsValue::LiteralName(key)) => {
                        let index = self.dict_find(identity, key, work)?.ok_or(E::UndefinedName(at))?;
                        self.dict_value(identity, index)
                    }
                    (EpsValue::Array { begin, length }, EpsValue::Number(EpsNumber::Integer(index))) => {
                        let index = usize::try_from(index).map_err(|_| E::Range)?;
                        if index >= length {
                            return Err(E::Range);
                        }
                        self.arrays[begin + index]
                    }
                    _ => return Err(E::Type),
                };
                self.push(value)?;
            }
            O::Repeat => {
                let proc = self.procedure()?;
                let n = usize::try_from(self.integer()?).map_err(|_| E::Range)?;
                self.call(proc, n)?;
                if n != 0 {
                    self.frames[self.depth - 1].continuation = Continuation::Repeat(n);
                }
            }
            O::Loop => {
                let proc = self.procedure()?;
                self.call(proc, 1)?;
                self.frames[self.depth - 1].continuation = Continuation::Loop;
            }
            O::Exit => {
                let mut target = None;
                for index in (0..self.depth).rev() {
                    work.tick()?;
                    if !matches!(self.frames[index].continuation, Continuation::Once) {
                        target = Some(index);
                        break;
                    }
                }
                self.depth = target.ok_or(E::InvalidExit)?;
            }
            O::For => {
                let proc = self.procedure()?;
                let mut limit = self.number()?;
                let mut increment = self.number()?;
                let mut current = self.number()?;
                // Adobe/Ghostscript compatibility: zero initial plus zero step
                // executes no body, including signed real zero.
                if as_double(current) == 0. && as_double(increment) == 0. {
                    return Ok(());
                }
                // Integer counters truncate a real limit toward zero. For a
                // real counter all three operands use the same real precision.
                if matches!(
                    (current, increment),
                    (EpsNumber::Integer(_), EpsNumber::Integer(_))
                ) {
                    if let EpsNumber::Real(value) = limit {
                        limit = EpsNumber::Real(value.trunc());
                    }
                } else {
                    current = EpsNumber::Real(as_real(current));
                    increment = EpsNumber::Real(as_real(increment));
                    limit = EpsNumber::Real(as_real(limit));
                }
                let range = ForLoop {
                    current,
                    increment,
                    limit,
                };
                if range.contains(current) {
                    self.call(proc, 1)?;
                    self.frames[self.depth - 1].continuation = Continuation::For(range);
                    self.push(EpsValue::Number(current))?;
                }
            }
            O::Eq | O::Ne => {
                let b = self.pop()?;
                let a = self.pop()?;
                let eq = equal(a, b);
                self.push(EpsValue::Boolean(if op == O::Eq { eq } else { !eq }))?;
            }
            O::Lt | O::Le | O::Gt | O::Ge => {
                let b = self.number()?;
                let a = self.number()?;
                let (a, b) = (as_double(a), as_double(b));
                self.push(EpsValue::Boolean(match op {
                    O::Lt => a < b,
                    O::Le => a <= b,
                    O::Gt => a > b,
                    _ => a >= b,
                }))?;
            }
            O::Not => {
                let value = match self.pop()? {
                    EpsValue::Boolean(b) => EpsValue::Boolean(!b),
                    EpsValue::Number(EpsNumber::Integer(n)) => EpsValue::Number(EpsNumber::Integer(!n)),
                    _ => return Err(E::Type),
                };
                self.push(value)?;
            }
            O::And | O::Or | O::Xor => {
                let b = self.pop()?;
                let a = self.pop()?;
                let value = match (a, b) {
                    (EpsValue::Boolean(a), EpsValue::Boolean(b)) => EpsValue::Boolean(match op {
                        O::And => a & b,
                        O::Or => a | b,
                        _ => a ^ b,
                    }),
                    (EpsValue::Number(EpsNumber::Integer(a)), EpsValue::Number(EpsNumber::Integer(b))) => {
                        EpsValue::Number(EpsNumber::Integer(match op {
                            O::And => a & b,
                            O::Or => a | b,
                            _ => a ^ b,
                        }))
                    }
                    _ => return Err(E::Type),
                };
                self.push(value)?;
            }
        }
        Ok(())
    }
    fn graphics_operation<F: FnMut() -> bool>(
        &mut self,
        op: EpsOperator,
        at: usize,
        work: &mut Work<F>,
    ) -> Result<(), EpsVmError> {
        use EpsOperator as O;
        if self.graphics.is_none() {
            return Err(EpsVmError::Unsupported(at));
        }
        let count = match op {
            O::MoveTo | O::RelativeMoveTo | O::LineTo | O::RelativeLineTo | O::Translate | O::Scale => 2,
            O::CurveTo | O::RelativeCurveTo => 6,
            O::Arc | O::ArcNegative => 5,
            O::SetRgbColor => 3,
            O::SetCmykColor | O::RectFill | O::RectStroke | O::RectClip => 4,
            O::Rotate | O::SetGray | O::SetLineWidth | O::SetLineCap | O::SetLineJoin | O::SetMiterLimit => 1,
            _ => 0,
        };
        let mut args = [0f64; 6];
        if op == O::Concat {
            let EpsValue::Array { begin, length } = self.pop()? else {
                return Err(EpsVmError::Type);
            };
            if length != 6 {
                return Err(EpsVmError::Range);
            }
            for (index, value) in args.iter_mut().enumerate() {
                work.tick()?;
                let EpsValue::Number(number) = self.arrays[begin + index] else {
                    return Err(EpsVmError::Type);
                };
                *value = as_double(number);
            }
        } else if matches!(op, O::SetLineCap | O::SetLineJoin) {
            args[0] = f64::from(u8::try_from(self.integer()?).map_err(|_| EpsVmError::Range)?);
        } else {
            for i in (0..count).rev() {
                args[i] = as_double(self.number()?);
            }
        }
        let graphics = self.graphics.as_deref_mut().ok_or(EpsVmError::Unsupported(at))?;
        let mut output = [0f64; 4];
        let mut output_count = 0;
        match op {
            O::NewPath => graphics.new_path(),
            O::MoveTo => graphics.move_to(args[0], args[1])?,
            O::RelativeMoveTo => graphics.relative_move_to(args[0], args[1])?,
            O::LineTo => graphics.line_to(args[0], args[1])?,
            O::RelativeLineTo => graphics.relative_line_to(args[0], args[1])?,
            O::CurveTo => graphics.curve_to(args)?,
            O::RelativeCurveTo => graphics.relative_curve_to(args)?,
            O::Arc | O::ArcNegative => {
                let mut failure = None;
                let result = graphics.arc(args[..5].try_into().unwrap(), op == O::ArcNegative, || match work
                    .tick()
                {
                    Ok(()) => false,
                    Err(error) => {
                        failure = Some(error);
                        true
                    }
                });
                if let Some(error) = failure {
                    return Err(error);
                }
                result?;
            }
            O::Clip | O::EvenOddClip => graphics.clip(op == O::EvenOddClip)?,
            O::InitClip => graphics.init_clip(),
            O::ClosePath => graphics.close_path()?,
            O::Stroke => graphics.paint(EpsPaintKind::Stroke)?,
            O::Fill => graphics.paint(EpsPaintKind::FillNonZero)?,
            O::EvenOddFill => graphics.paint(EpsPaintKind::FillEvenOdd)?,
            O::RectClip => graphics.clip_rectangle(args[0], args[1], args[2], args[3])?,
            O::RectFill | O::RectStroke => graphics.paint_rectangle(
                args[0],
                args[1],
                args[2],
                args[3],
                if op == O::RectFill {
                    EpsPaintKind::FillNonZero
                } else {
                    EpsPaintKind::Stroke
                },
            )?,
            O::SetGray => graphics.set_color(EpsDeviceColor::Gray(args[0]))?,
            O::SetRgbColor => graphics.set_color(EpsDeviceColor::Rgb([args[0], args[1], args[2]]))?,
            O::SetCmykColor => {
                graphics.set_color(EpsDeviceColor::Cmyk([args[0], args[1], args[2], args[3]]))?
            }
            O::SetLineWidth => graphics.set_line_width(args[0])?,
            O::SetLineCap => graphics.set_line_cap(args[0] as u8)?,
            O::SetLineJoin => graphics.set_line_join(args[0] as u8)?,
            O::SetMiterLimit => graphics.set_miter_limit(args[0])?,
            O::GSave => graphics.gsave()?,
            O::GRestore => graphics.grestore()?,
            O::GRestoreAll => {
                let mut failure = None;
                let result = graphics.grestore_all(|| match work.tick() {
                    Ok(()) => false,
                    Err(error) => {
                        failure = Some(error);
                        true
                    }
                });
                if let Some(error) = failure {
                    return Err(error);
                }
                result?;
            }
            O::Translate => graphics.translate(args[0], args[1])?,
            O::Scale => graphics.scale(args[0], args[1])?,
            O::Rotate => graphics.rotate(args[0])?,
            O::InitMatrix => graphics.init_matrix(),
            O::Concat => graphics.concat(args)?,
            O::InitGraphics => graphics.init_graphics(),
            O::CurrentPoint => {
                output[..2].copy_from_slice(&graphics.current_point()?);
                output_count = 2;
            }
            O::PathBBox => {
                let mut failure = None;
                let bounds = graphics.path_bbox(|| match work.tick() {
                    Ok(()) => false,
                    Err(e) => {
                        failure = Some(e);
                        true
                    }
                });
                if let Some(error) = failure {
                    return Err(error);
                }
                output = bounds?;
                output_count = 4;
            }
            O::CurrentLineWidth => {
                output[0] = graphics.style().width;
                output_count = 1;
            }
            O::CurrentLineCap => {
                output[0] = f64::from(graphics.style().cap);
                output_count = 1;
            }
            O::CurrentLineJoin => {
                output[0] = f64::from(graphics.style().join);
                output_count = 1;
            }
            O::CurrentMiterLimit => {
                output[0] = graphics.style().miter_limit;
                output_count = 1;
            }
            O::CurrentGray => {
                let EpsDeviceColor::Gray(n) = graphics.style().color else {
                    return Err(EpsVmError::Unsupported(at));
                };
                output[0] = n;
                output_count = 1;
            }
            O::CurrentRgbColor => {
                let EpsDeviceColor::Rgb(v) = graphics.style().color else {
                    return Err(EpsVmError::Unsupported(at));
                };
                output[..3].copy_from_slice(&v);
                output_count = 3;
            }
            O::CurrentCmykColor => {
                let EpsDeviceColor::Cmyk(v) = graphics.style().color else {
                    return Err(EpsVmError::Unsupported(at));
                };
                output = v;
                output_count = 4;
            }
            _ => return Err(EpsVmError::Unsupported(at)),
        }
        for n in output.iter().take(output_count) {
            let value = if matches!(op, O::CurrentLineCap | O::CurrentLineJoin) {
                EpsNumber::Integer(*n as i32)
            } else {
                let n = *n as f32;
                if !n.is_finite() {
                    return Err(EpsVmError::Range);
                }
                EpsNumber::Real(n)
            };
            self.push(EpsValue::Number(value))?;
        }
        Ok(())
    }
    fn execute<F: FnMut() -> bool>(
        &mut self,
        value: EpsValue<'a>,
        at: usize,
        work: &mut Work<F>,
    ) -> Result<(), EpsVmError> {
        let mut value = value;
        loop {
            work.tick()?;
            match value {
                EpsValue::Operator(EpsOperator::Exec) => value = self.pop()?,
                p @ EpsValue::Procedure { .. } => return self.call(p, 1),
                EpsValue::Operator(op) => return self.operate(op, at, work),
                other => return self.push(other),
            }
        }
    }
}
fn as_real(n: EpsNumber) -> f32 {
    match n {
        EpsNumber::Integer(n) => n as f32,
        EpsNumber::Real(n) => n,
    }
}
fn as_double(n: EpsNumber) -> f64 {
    match n {
        EpsNumber::Integer(n) => f64::from(n),
        EpsNumber::Real(n) => f64::from(n),
    }
}
fn equality_pair(a: EpsNumber, b: EpsNumber) -> (f64, f64) {
    match (a, b) {
        (EpsNumber::Integer(a), EpsNumber::Integer(b)) => (f64::from(a), f64::from(b)),
        _ => (f64::from(as_real(a)), f64::from(as_real(b))),
    }
}
fn arithmetic(op: EpsOperator, a: EpsNumber, b: EpsNumber) -> Result<EpsNumber, EpsVmError> {
    use EpsOperator as O;
    if let (EpsNumber::Integer(a), EpsNumber::Integer(b)) = (a, b) {
        let integer = match op {
            O::Add => a.checked_add(b),
            O::Sub => a.checked_sub(b),
            O::Mul => a.checked_mul(b),
            _ => None,
        };
        if let Some(n) = integer {
            return Ok(EpsNumber::Integer(n));
        }
        // Integer overflow converts the exact mathematical integer result to real.
        let wide = match op {
            O::Add => Some(i64::from(a) + i64::from(b)),
            O::Sub => Some(i64::from(a) - i64::from(b)),
            O::Mul => Some(i64::from(a) * i64::from(b)),
            _ => None,
        };
        if let Some(n) = wide {
            return Ok(EpsNumber::Real(n as f32));
        }
    }
    let a = as_real(a);
    let b = as_real(b);
    if op == O::Div && b == 0.0 {
        return Err(EpsVmError::UndefinedResult);
    }
    let n = match op {
        O::Add => a + b,
        O::Sub => a - b,
        O::Mul => a * b,
        O::Div => a / b,
        _ => return Err(EpsVmError::Type),
    };
    if !n.is_finite() {
        return Err(EpsVmError::UndefinedResult);
    }
    Ok(EpsNumber::Real(n))
}
fn equal(a: EpsValue<'_>, b: EpsValue<'_>) -> bool {
    let empty_array = |value| match value {
        EpsValue::Array { length: 0, .. } => true,
        EpsValue::Procedure { begin, end, .. } => begin == end,
        _ => false,
    };
    if empty_array(a) && empty_array(b) {
        return true;
    }
    match (a, b) {
        (EpsValue::Number(a), EpsValue::Number(b)) => {
            let (a, b) = equality_pair(a, b);
            a == b
        }
        (EpsValue::Procedure { identity: a, .. }, EpsValue::Procedure { identity: b, .. }) => a == b,
        _ => a == b,
    }
}
fn operator(word: &[u8]) -> Option<EpsOperator> {
    use EpsOperator as O;
    Some(match word {
        b"dict" => O::Dict,
        b"begin" => O::Begin,
        b"end" => O::End,
        b"currentdict" => O::CurrentDict,
        b"known" => O::Known,
        b"save" => O::Save,
        b"restore" => O::Restore,
        b"add" => O::Add,
        b"sub" => O::Sub,
        b"mul" => O::Mul,
        b"div" => O::Div,
        b"idiv" => O::IntegerDiv,
        b"mod" => O::Remainder,
        b"sqrt" => O::Sqrt,
        b"ln" => O::Ln,
        b"log" => O::Log,
        b"cvi" => O::ConvertInteger,
        b"cvr" => O::ConvertReal,
        b"sin" => O::Sin,
        b"cos" => O::Cos,
        b"atan" => O::Atan,
        b"exp" => O::Exp,
        b"neg" => O::Neg,
        b"abs" => O::Abs,
        b"ceiling" => O::Ceiling,
        b"floor" => O::Floor,
        b"round" => O::Round,
        b"truncate" => O::Truncate,
        b"dup" => O::Dup,
        b"exch" => O::Exch,
        b"pop" => O::Pop,
        b"clear" => O::Clear,
        b"count" => O::Count,
        b"mark" | b"[" => O::Mark,
        b"]" => O::ArrayEnd,
        b"array" => O::Array,
        b"put" => O::Put,
        b"aload" => O::ALoad,
        b"astore" => O::AStore,
        b"getinterval" => O::GetInterval,
        b"putinterval" => O::PutInterval,
        b"length" => O::Length,
        b"get" => O::Get,
        b"index" => O::Index,
        b"copy" => O::Copy,
        b"roll" => O::Roll,
        b"def" => O::Def,
        b"load" => O::Load,
        b"bind" => O::Bind,
        b"exec" => O::Exec,
        b"if" => O::If,
        b"ifelse" => O::IfElse,
        b"repeat" => O::Repeat,
        b"for" => O::For,
        b"loop" => O::Loop,
        b"exit" => O::Exit,
        b"eq" => O::Eq,
        b"ne" => O::Ne,
        b"lt" => O::Lt,
        b"le" => O::Le,
        b"gt" => O::Gt,
        b"ge" => O::Ge,
        b"not" => O::Not,
        b"and" => O::And,
        b"or" => O::Or,
        b"xor" => O::Xor,
        b"newpath" => O::NewPath,
        b"moveto" => O::MoveTo,
        b"rmoveto" => O::RelativeMoveTo,
        b"lineto" => O::LineTo,
        b"rlineto" => O::RelativeLineTo,
        b"curveto" => O::CurveTo,
        b"rcurveto" => O::RelativeCurveTo,
        b"arc" => O::Arc,
        b"arcn" => O::ArcNegative,
        b"clip" => O::Clip,
        b"eoclip" => O::EvenOddClip,
        b"initclip" => O::InitClip,
        b"closepath" => O::ClosePath,
        b"stroke" => O::Stroke,
        b"fill" => O::Fill,
        b"eofill" => O::EvenOddFill,
        b"rectfill" => O::RectFill,
        b"rectstroke" => O::RectStroke,
        b"rectclip" => O::RectClip,
        b"setgray" => O::SetGray,
        b"setrgbcolor" => O::SetRgbColor,
        b"setcmykcolor" => O::SetCmykColor,
        b"setlinewidth" => O::SetLineWidth,
        b"setlinecap" => O::SetLineCap,
        b"setlinejoin" => O::SetLineJoin,
        b"setmiterlimit" => O::SetMiterLimit,
        b"gsave" => O::GSave,
        b"grestore" => O::GRestore,
        b"grestoreall" => O::GRestoreAll,
        b"translate" => O::Translate,
        b"scale" => O::Scale,
        b"rotate" => O::Rotate,
        b"initmatrix" => O::InitMatrix,
        b"concat" => O::Concat,
        b"initgraphics" => O::InitGraphics,
        b"currentpoint" => O::CurrentPoint,
        b"pathbbox" => O::PathBBox,
        b"currentlinewidth" => O::CurrentLineWidth,
        b"currentlinecap" => O::CurrentLineCap,
        b"currentlinejoin" => O::CurrentLineJoin,
        b"currentmiterlimit" => O::CurrentMiterLimit,
        b"currentgray" => O::CurrentGray,
        b"currentrgbcolor" => O::CurrentRgbColor,
        b"currentcmykcolor" => O::CurrentCmykColor,

        _ => return None,
    })
}
/// Execute the scalar/procedure subset of a lexical code program. All operand,
/// dictionary and execution-frame capacities are admitted to the supplied root
/// before execution; no heap-growing stacks or recursive Rust calls are used.
/// The result retains its full operand capacity until the last shared owner.
/// Names borrow original source bytes. Procedure values are diagnostic references
/// to this program's instruction ranges, not standalone executable objects.
/// Name-keyed dictionaries use a bounded shared record arena and stack. Full
/// system dictionaries, non-name dictionary keys, strings, immediate names, stream reading,
/// graphics operators require evaluate_eps_vectors; unsupported device operators
/// are never silently ignored.
pub fn evaluate_eps_program<'a, F: FnMut() -> bool>(
    program: &EpsProgram<'a>,
    limits: EpsVmLimits,
    budget: &MemoryBudget,
    cancelled: F,
) -> Result<EpsEvaluation<'a>, EpsVmError> {
    evaluate_eps_impl(program, limits, budget, cancelled, None)
}

fn evaluate_eps_impl<'a, F: FnMut() -> bool>(
    program: &EpsProgram<'a>,
    limits: EpsVmLimits,
    budget: &MemoryBudget,
    mut cancelled: F,
    graphics: Option<&mut EpsGraphics>,
) -> Result<EpsEvaluation<'a>, EpsVmError> {
    if cancelled() {
        return Err(EpsVmError::Cancelled);
    }
    if limits.max_execution_frames == 0 {
        return Err(EpsVmError::Limit(EpsVmLimit::ExecutionFrames));
    }
    let operands = budget.try_buffer(limits.max_operands, EpsValue::Null)?;
    if cancelled() {
        return Err(EpsVmError::Cancelled);
    }
    let dictionary = budget.try_buffer(
        limits.max_dictionary_entries,
        Entry {
            key: None,
            value: EpsValue::Null,
        },
    )?;
    if cancelled() {
        return Err(EpsVmError::Cancelled);
    }
    let frames = budget.try_buffer(
        limits.max_execution_frames,
        Frame {
            begin: 0,
            pc: 0,
            end: 0,
            continuation: Continuation::Once,
        },
    )?;
    if cancelled() {
        return Err(EpsVmError::Cancelled);
    }
    // Admit the full bounded arena when the first array is constructed. Scalar
    // programs keep their original storage cost; no incremental growth occurs.
    let arrays = budget.try_buffer(0, EpsValue::Null)?;
    if cancelled() {
        return Err(EpsVmError::Cancelled);
    }
    let tape = program.instructions();
    let mut machine = Machine {
        tape,
        saves: None,
        save_depth: 0,
        next_save: 1,
        dict_stack: budget.try_buffer(0, EpsValue::Null)?,
        dict_depth: 0,
        retain_user_dictionary: false,
        bindings: budget.try_buffer(0, Binding::default())?,
        graphics,
        arrays,
        array_used: 0,
        array_capacity: limits.max_operands,
        array_budget: budget.clone(),
        operands,
        length: 0,
        dictionary,
        dict_records: budget.try_buffer(0, DictRecord::default())?,
        dict_used: 0,
        frames,
        depth: 1,
    };
    machine.frames[0] = Frame {
        begin: 0,
        pc: 0,
        end: tape.len(),
        continuation: Continuation::Once,
    };
    let mut work = Work {
        remaining: limits.max_work,
        cancelled,
    };
    while machine.depth != 0 {
        work.tick()?;
        let frame = &mut machine.frames[machine.depth - 1];
        if frame.pc == frame.end {
            match frame.continuation {
                Continuation::For(mut range) => {
                    if let Some(next) = range.next()? {
                        range.current = next;
                        frame.continuation = Continuation::For(range);
                        frame.pc = frame.begin;
                        machine.push(EpsValue::Number(next))?;
                    } else {
                        machine.depth -= 1;
                    }
                }
                Continuation::Repeat(n) if n > 1 => {
                    frame.continuation = Continuation::Repeat(n - 1);
                    frame.pc = frame.begin;
                }
                Continuation::Loop => frame.pc = frame.begin,
                _ => machine.depth -= 1,
            }
            continue;
        }
        let at = frame.pc;
        frame.pc += 1;
        let instruction = tape[at];
        match instruction.token {
            EpsToken::Word(word) => {
                if let Some(number) = instruction.number {
                    machine.push(EpsValue::Number(number))?;
                } else {
                    let value = if let Some(op) = machine.bindings.get(at).and_then(|b| b.operator) {
                        EpsValue::Operator(op)
                    } else {
                        machine.lookup(word, at, &mut work)?
                    };
                    machine.execute(value, at, &mut work)?;
                }
            }
            EpsToken::LiteralName(name) => machine.push(EpsValue::LiteralName(name))?,
            EpsToken::ArrayStart => machine.operate(EpsOperator::Mark, at, &mut work)?,
            EpsToken::ArrayEnd => machine.operate(EpsOperator::ArrayEnd, at, &mut work)?,
            EpsToken::ProcedureStart => {
                let end = instruction.procedure_partner.ok_or(EpsVmError::Unsupported(at))?;
                // Immediate names must resolve when scanning a procedure, not on
                // later invocation; refuse until native object capture supports it.
                for (offset, body) in tape[at + 1..end].iter().enumerate() {
                    work.tick()?;
                    if matches!(body.token, EpsToken::ImmediateName(_)) {
                        return Err(EpsVmError::Unsupported(at + 1 + offset));
                    }
                }
                machine.frames[machine.depth - 1].pc = end + 1;
                machine.push(EpsValue::Procedure {
                    begin: at + 1,
                    end,
                    identity: if at + 1 == end {
                        0
                    } else {
                        at.checked_add(1).ok_or(EpsVmError::Range)?
                    },
                })?;
            }
            _ => return Err(EpsVmError::Unsupported(at)),
        }
    }
    if (work.cancelled)() {
        return Err(EpsVmError::Cancelled);
    }
    Ok(EpsEvaluation {
        operands: machine.operands.freeze(),
        length: machine.length,
        _arrays: machine.arrays.freeze(),
        _dict_records: machine.dict_records.freeze(),
        _user_dictionary: if machine.retain_user_dictionary {
            Some(machine.dictionary.freeze())
        } else {
            None
        },
    })
}

#[derive(Debug, Clone)]
pub struct EpsVectorExecution<'a> {
    pub evaluation: EpsEvaluation<'a>,
    pub scene: EpsVectorScene,
}
/// Execute supported scalar and graphics operators into a managed vector scene.
/// Program, source, VM storage and graphics storage must share a caller-selected
/// root budget. Failed execution destroys the fresh partial scene. Successful
/// output retains its operand capacity and vector arena until the last owners.
/// This is lexical-code execution, not stream-aware full EPS decoding. It does
/// not rasterize, resolve fonts/images/clips or convert between device color
/// families; unsupported behavior refuses without returning partial artwork.
pub fn evaluate_eps_vectors<'a, F: FnMut() -> bool>(
    program: &EpsProgram<'a>,
    vm_limits: EpsVmLimits,
    graphics_limits: EpsGraphicsLimits,
    budget: &MemoryBudget,
    mut cancelled: F,
) -> Result<EpsVectorExecution<'a>, EpsVmError> {
    let mut graphics = EpsGraphics::new(graphics_limits, budget, &mut cancelled).map_err(|e| match e {
        EpsGraphicsError::Cancelled => EpsVmError::Cancelled,
        other => EpsVmError::Graphics(other),
    })?;
    let evaluation = evaluate_eps_impl(program, vm_limits, budget, &mut cancelled, Some(&mut graphics))?;
    if cancelled() {
        return Err(EpsVmError::Cancelled);
    }
    Ok(EpsVectorExecution {
        evaluation,
        scene: graphics.finish(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EpsCompileLimits, EpsSource, compile_eps_program};
    fn compile<'a>(bytes: &'a [u8], budget: &MemoryBudget) -> EpsProgram<'a> {
        compile_eps_program(
            &EpsSource {
                postscript: bytes,
                wmf_preview: None,
                tiff_preview: None,
            },
            EpsCompileLimits::default(),
            budget,
            || false,
        )
        .unwrap()
    }
    fn run(bytes: &[u8]) -> Vec<EpsValue<'_>> {
        let budget = MemoryBudget::new(1_000_000);
        let program = compile(bytes, &budget);
        evaluate_eps_program(&program, EpsVmLimits::default(), &budget, || false)
            .unwrap()
            .values()
            .to_vec()
    }
    fn int(n: i32) -> EpsValue<'static> {
        EpsValue::Number(EpsNumber::Integer(n))
    }
    #[test]
    fn bind_matches_independent_ghostscript_operators_aliases_and_readonly_nesting() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/bind-ghostscript-reference.json"
        ))
        .unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let code = case["code"].as_str().unwrap();
            assert_eq!(
                run(code.as_bytes()),
                vec![int(case["integer"].as_i64().unwrap() as i32)],
                "{code}"
            );
        }
    }
    #[test]
    fn bound_graphics_prolog_preserves_all_pixels_against_ghostscript() {
        let root = MemoryBudget::new(32 * 1024 * 1024);
        let raster = crate::decode_eps_document(
            include_bytes!("../../../tests/fixtures/eps/bind-prolog.ai"),
            1.,
            crate::EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
            crate::EpsDocumentLimits::default(),
            &root,
            || false,
        )
        .unwrap();
        assert_eq!((raster.width(), raster.height()), (8, 8));
        let rrrah_core::RasterPixels::Rgba8(pixels) = raster.pixels() else {
            panic!()
        };
        let reference = include_bytes!("../../../tests/fixtures/eps/bind-prolog.rgb");
        for (index, pixel) in pixels.chunks_exact(4).enumerate() {
            let x = index % 8;
            let y = index / 8;
            assert_eq!(
                pixel[3],
                if (1..7).contains(&x) && (1..7).contains(&y) {
                    255
                } else {
                    0
                }
            );
            for c in 0..3 {
                assert_eq!(
                    (u32::from(pixel[c]) * u32::from(pixel[3]) + 255 * (255 - u32::from(pixel[3])) + 127)
                        / 255,
                    u32::from(reference[index * 3 + c])
                );
            }
        }
        drop(raster);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn bind_memory_cancellation_and_work_failure_are_bounded_and_retryable() {
        let root = MemoryBudget::new(1_000_000);
        let program = compile(b"/p {{add} exec 2 mul} bind def 4 5 p", &root);
        let baseline = root.used();
        let retained = evaluate_eps_program(&program, EpsVmLimits::default(), &root, || false).unwrap();
        assert_eq!(retained.values(), &[int(18)]);
        drop(retained);
        assert_eq!(root.used(), baseline);
        let polls = std::cell::Cell::new(0);
        drop(
            evaluate_eps_program(&program, EpsVmLimits::default(), &root, || {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap(),
        );
        for stop in 1..=polls.get() {
            let mut n = 0;
            assert!(matches!(
                evaluate_eps_program(&program, EpsVmLimits::default(), &root, || {
                    n += 1;
                    n == stop
                }),
                Err(EpsVmError::Cancelled)
            ));
            assert_eq!(root.used(), baseline);
        }
        for max_work in 0..32 {
            let result = evaluate_eps_program(
                &program,
                EpsVmLimits {
                    max_work,
                    ..EpsVmLimits::default()
                },
                &root,
                || false,
            );
            assert!(matches!(result, Err(EpsVmError::Limit(EpsVmLimit::Work))));
            assert_eq!(root.used(), baseline);
        }
        let limits = EpsVmLimits::default();
        let vm_bytes = limits.max_operands * std::mem::size_of::<EpsValue<'_>>()
            + limits.max_dictionary_entries * std::mem::size_of::<Entry<'_>>()
            + limits.max_execution_frames * std::mem::size_of::<Frame>();
        let bind_bytes = program.instructions().len() * std::mem::size_of::<Binding>();
        let required = (vm_bytes + bind_bytes) as u64;
        let tight = root.try_reserve(root.available_bytes() - (required - 1)).unwrap();
        assert!(matches!(
            evaluate_eps_program(&program, limits, &root, || false),
            Err(EpsVmError::Memory(_))
        ));
        drop(tight);
        assert_eq!(root.used(), baseline);
        let exact = root.try_reserve(root.available_bytes() - required).unwrap();
        drop(evaluate_eps_program(&program, limits, &root, || false).unwrap());
        drop(exact);
        assert_eq!(root.used(), baseline);
        let pressure = root.try_reserve(root.available_bytes()).unwrap();
        assert!(matches!(
            evaluate_eps_program(&program, EpsVmLimits::default(), &root, || false),
            Err(EpsVmError::Memory(_))
        ));
        drop(pressure);
        drop(evaluate_eps_program(&program, EpsVmLimits::default(), &root, || false).unwrap());
        drop(program);
        assert_eq!(root.used(), 0);
        assert!(matches!(
            {
                let p = compile(b"1 bind", &root);
                evaluate_eps_program(&p, EpsVmLimits::default(), &root, || false)
            },
            Err(EpsVmError::Type)
        ));
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn array_bulk_views_and_overlapping_copies_match_reference() {
        assert_eq!(
            run(b"1 2 3 3 array astore aload pop"),
            vec![int(1), int(2), int(3)]
        );
        assert_eq!(
            run(b"/a [0 1 2 3] def /v a 1 2 getinterval def v 0 9 put a aload pop"),
            vec![int(0), int(9), int(2), int(3)]
        );
        assert_eq!(
            run(b"/a [0 1 2 3 4] def a 1 a 0 4 getinterval putinterval a aload pop"),
            vec![int(0), int(0), int(1), int(2), int(3)]
        );
        assert_eq!(
            run(b"/a [0 1 2 3 4] def a 0 a 1 4 getinterval putinterval a aload pop"),
            vec![int(1), int(2), int(3), int(4), int(4)]
        );
        assert_eq!(
            run(b"/a [1 2] def /b 4 array def a b copy length b aload pop"),
            vec![int(2), int(1), int(2), EpsValue::Null, EpsValue::Null]
        );
        assert_eq!(
            run(b"/a [1 2] def /b 4 array def /r a b copy def r b 0 2 getinterval eq r 1 9 put b 1 get"),
            vec![EpsValue::Boolean(true), int(9)]
        );
    }
    #[test]
    fn array_bulk_errors_and_partial_copy_cancellation_release_credit() {
        let root = MemoryBudget::new(1_000_000);
        for code in [
            b"[1] 0 2 getinterval".as_slice(),
            b"[1] -1 0 getinterval",
            b"[1] 1 [2] putinterval",
            b"[1 2] 1 array copy",
            b"1 2 3 array astore",
        ] {
            let p = compile(code, &root);
            let base = root.used();
            assert!(evaluate_eps_program(&p, EpsVmLimits::default(), &root, || false).is_err());
            assert_eq!(root.used(), base);
            drop(p);
        }
        let p = compile(b"99 4 array aload", &root);
        let base = root.used();
        assert!(matches!(
            evaluate_eps_program(
                &p,
                EpsVmLimits {
                    max_operands: 5,
                    ..EpsVmLimits::default()
                },
                &root,
                || false
            ),
            Err(EpsVmError::Limit(EpsVmLimit::Operands))
        ));
        assert_eq!(root.used(), base);
        drop(p);
        let p = compile(
            b"/a [0 1 2 3 4] def a 1 a 0 4 getinterval putinterval a aload pop",
            &root,
        );
        let base = root.used();
        let mut ticks = 0;
        let result = evaluate_eps_program(&p, EpsVmLimits::default(), &root, || {
            ticks += 1;
            false
        })
        .unwrap();
        assert_eq!(result.values(), &[int(0), int(0), int(1), int(2), int(3)]);
        drop(result);
        for stop in 1..=ticks {
            let mut at = 0;
            assert!(matches!(
                evaluate_eps_program(&p, EpsVmLimits::default(), &root, || {
                    at += 1;
                    at == stop
                }),
                Err(EpsVmError::Cancelled)
            ));
            assert_eq!(root.used(), base);
        }
        let result = evaluate_eps_program(&p, EpsVmLimits::default(), &root, || false).unwrap();
        drop(result);
        drop(p);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn array_allocation_put_aliases_and_cycles_match_reference() {
        assert_eq!(
            run(b"3 array dup length exch 0 get"),
            vec![int(3), EpsValue::Null]
        );
        assert_eq!(run(b"/a [1 2] def /b a def b 1 9 put a 1 get"), vec![int(9)]);
        assert_eq!(run(b"/a 2 array def a 0 [3 4] put a 0 get 1 get"), vec![int(4)]);
        assert_eq!(
            run(b"/a 1 array def a 0 a put a 0 get a eq"),
            vec![EpsValue::Boolean(true)]
        );
    }
    #[test]
    fn array_mutation_errors_cancellation_and_cycle_drop_preserve_credit() {
        let root = MemoryBudget::new(1_000_000);
        for code in [
            b"-1 array".as_slice(),
            b"true array",
            b"[1] 1 9 put",
            b"[1] -1 9 put",
            b"false 0 1 put",
        ] {
            let p = compile(code, &root);
            let base = root.used();
            assert!(evaluate_eps_program(&p, EpsVmLimits::default(), &root, || false).is_err());
            assert_eq!(root.used(), base);
            drop(p);
            assert_eq!(root.used(), 0);
        }
        let p = compile(b"/a 1 array def a 0 a put a", &root);
        let base = root.used();
        let mut ticks = 0;
        let result = evaluate_eps_program(&p, EpsVmLimits::default(), &root, || {
            ticks += 1;
            false
        })
        .unwrap();
        let last = result.clone();
        drop(result);
        assert!(root.used() > base);
        drop(last);
        assert_eq!(root.used(), base);
        for stop in 1..=ticks {
            let mut at = 0;
            assert!(matches!(
                evaluate_eps_program(&p, EpsVmLimits::default(), &root, || {
                    at += 1;
                    at == stop
                }),
                Err(EpsVmError::Cancelled)
            ));
            assert_eq!(root.used(), base);
        }
        let result = evaluate_eps_program(&p, EpsVmLimits::default(), &root, || false).unwrap();
        drop(result);
        drop(p);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn literal_arrays_evaluate_elements_and_retain_nested_identities() {
        assert_eq!(
            run(b"[1 2 add [4 5] true] dup length exch 0 get"),
            vec![int(3), int(3)]
        );
        assert_eq!(
            run(b"/a [1 [7 8]] def a 1 get 0 get a dup eq [] [] eq"),
            vec![int(7), EpsValue::Boolean(true), EpsValue::Boolean(true)]
        );
        assert_eq!(
            run(b"[] {} eq [1] [1] eq"),
            vec![EpsValue::Boolean(true), EpsValue::Boolean(false)]
        );
        assert_eq!(run(b"[0 1 3 {} for] dup length exch 3 get"), vec![int(4), int(3)]);
    }

    #[test]
    fn array_limits_errors_and_last_owner_release_the_admitted_arena() {
        let root = MemoryBudget::new(1_000_000);
        let good = compile(b"[1 [2 3]]", &root);
        let bad = compile(b"]", &root);
        let range = compile(b"[1] 1 get", &root);
        let pressure = compile(b"3 {[] pop} repeat", &root);
        let base = root.used();
        for program in [&bad, &range] {
            assert!(evaluate_eps_program(program, EpsVmLimits::default(), &root, || false).is_err());
            assert_eq!(root.used(), base);
        }
        assert!(matches!(
            evaluate_eps_program(
                &pressure,
                EpsVmLimits {
                    max_operands: 2,
                    ..EpsVmLimits::default()
                },
                &root,
                || false
            ),
            Err(EpsVmError::Limit(EpsVmLimit::ArrayItems))
        ));
        assert_eq!(root.used(), base);
        let mut admitted = false;
        assert!(matches!(
            evaluate_eps_program(&good, EpsVmLimits::default(), &root, || {
                admitted |= root.used() > base;
                admitted
            }),
            Err(EpsVmError::Cancelled)
        ));
        assert!(admitted);
        assert_eq!(root.used(), base);
        let limits = EpsVmLimits::default();
        let before_arena = limits.max_operands * std::mem::size_of::<EpsValue<'_>>()
            + limits.max_dictionary_entries * std::mem::size_of::<Entry<'_>>()
            + limits.max_execution_frames * std::mem::size_of::<Frame>();
        let mut arena_admitted = false;
        assert!(matches!(
            evaluate_eps_program(&good, limits, &root, || {
                arena_admitted |= root.used() > base + before_arena as u64;
                arena_admitted
            }),
            Err(EpsVmError::Cancelled)
        ));
        assert!(arena_admitted);
        assert_eq!(root.used(), base);
        let result = evaluate_eps_program(&good, EpsVmLimits::default(), &root, || false).unwrap();
        let last = result.clone();
        drop(result);
        assert!(root.used() > base);
        drop(last);
        assert_eq!(root.used(), base);
        drop((good, bad, range, pressure));
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn loop_exit_unwinds_only_the_nearest_loop_through_procedures() {
        assert_eq!(run(b"0 {1 add dup 3 eq {exit} if} loop 9"), vec![int(3), int(9)]);
        assert_eq!(run(b"0 2 {3 {1 add exit} repeat 10 add} repeat"), vec![int(22)]);
        assert_eq!(
            run(b"1 1 5 {dup 3 eq {exit} if} for 9"),
            vec![int(1), int(2), int(3), int(9)]
        );
        assert_eq!(run(b"1 {{exit} exec 100} repeat 9"), vec![int(9)]);
        assert_eq!(run(b"/leave {exit} def 5 {leave 100} repeat 9"), vec![int(9)]);
    }

    #[test]
    fn loop_work_and_invalid_exit_failures_release_admission() {
        let root = MemoryBudget::new(1_000_000);
        let endless = compile(b"{} loop", &root);
        let invalid = compile(b"{exit} exec", &root);
        let finite = compile(b"0 {1 add dup 3 eq {exit} if} loop", &root);
        let base = root.used();
        let limits = EpsVmLimits {
            max_work: 200,
            ..EpsVmLimits::default()
        };
        assert!(matches!(
            evaluate_eps_program(&endless, limits, &root, || false),
            Err(EpsVmError::Limit(EpsVmLimit::Work))
        ));
        assert_eq!(root.used(), base);
        assert!(matches!(
            evaluate_eps_program(&invalid, limits, &root, || false),
            Err(EpsVmError::InvalidExit)
        ));
        assert_eq!(root.used(), base);
        let mut admitted = false;
        assert!(matches!(
            evaluate_eps_program(&endless, limits, &root, || {
                admitted |= root.used() > base;
                admitted
            }),
            Err(EpsVmError::Cancelled)
        ));
        assert!(admitted);
        assert_eq!(root.used(), base);
        let result = evaluate_eps_program(&finite, EpsVmLimits::default(), &root, || false).unwrap();
        assert_eq!(result.values(), &[int(3)]);
        drop(result);
        assert_eq!(root.used(), base);
        drop((endless, invalid, finite));
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn for_numeric_sequences_and_nested_procedures_match_reference() {
        assert_eq!(run(b"1 1 3.5 {} for"), vec![int(1), int(2), int(3)]);
        assert_eq!(run(b"3 -1 1 {} for"), vec![int(3), int(2), int(1)]);
        assert_eq!(run(b"3 1 1 {} for 1 -1 3 {} for 9"), vec![int(9)]);
        assert_eq!(
            run(b"1 0.5 2 {} for"),
            vec![
                EpsValue::Number(EpsNumber::Real(1.)),
                EpsValue::Number(EpsNumber::Real(1.5)),
                EpsValue::Number(EpsNumber::Real(2.)),
            ]
        );
        assert_eq!(
            run(b"2147483646 1 2147483647 {} for"),
            vec![int(2147483646), int(2147483647)]
        );
        assert_eq!(run(b"0 1 1 3 {2 {dup} repeat add add add} for"), vec![int(18)]);
    }

    #[test]
    fn for_zero_initial_and_fractional_integer_limits_match_reference() {
        assert_eq!(run(b"0 0 1 {100} for 9"), vec![int(9)]);
        assert_eq!(run(b"-0.0 0.0 1 {100} for 9"), vec![int(9)]);
        assert_eq!(run(b"0 1 -0.5 {} for"), vec![int(0)]);
        assert_eq!(run(b"0 -1 0.5 {} for"), vec![int(0)]);
    }

    #[test]
    fn for_fractional_steps_match_independent_real_trace() {
        let expected = [
            0.,
            0.1,
            0.2,
            0.3,
            0.4,
            0.5,
            0.6,
            0.700000048,
            0.800000072,
            0.900000095,
        ];
        assert_eq!(
            run(b"0 0.1 1 {} for"),
            expected.map(|n| EpsValue::Number(EpsNumber::Real(n))).to_vec()
        );
        let expected = [
            1.,
            0.9,
            0.799999952,
            0.699999928,
            0.599999905,
            0.499999911,
            0.399999917,
            0.299999923,
            0.199999928,
            0.099999927,
        ];
        assert_eq!(
            run(b"1 -0.1 0 {} for"),
            expected.map(|n| EpsValue::Number(EpsNumber::Real(n))).to_vec()
        );
        let root = MemoryBudget::new(1_000_000);
        let program = compile(b"2147483647 0.5 2147483647 {pop} for", &root);
        let base = root.used();
        assert!(matches!(
            evaluate_eps_program(
                &program,
                EpsVmLimits {
                    max_work: 200,
                    ..EpsVmLimits::default()
                },
                &root,
                || false
            ),
            Err(EpsVmError::Limit(EpsVmLimit::Work))
        ));
        assert_eq!(root.used(), base);
        drop(program);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn for_limits_cancellation_and_integer_endpoints_release_temporary_credit() {
        assert_eq!(
            run(b"-2147483647 -1 -2147483648 {} for"),
            vec![int(-2147483647), int(i32::MIN)]
        );
        let root = MemoryBudget::new(1_000_000);
        let finite = compile(b"0 1 5 {pop} for", &root);
        let zero = compile(b"1 0 2 {pop} for", &root);
        let base = root.used();
        let limits = EpsVmLimits {
            max_work: 200,
            ..EpsVmLimits::default()
        };
        assert!(matches!(
            evaluate_eps_program(&zero, limits, &root, || false),
            Err(EpsVmError::Limit(EpsVmLimit::Work))
        ));
        assert_eq!(root.used(), base);
        assert!(matches!(
            evaluate_eps_program(
                &finite,
                EpsVmLimits {
                    max_execution_frames: 1,
                    ..limits
                },
                &root,
                || false
            ),
            Err(EpsVmError::Limit(EpsVmLimit::ExecutionFrames))
        ));
        assert_eq!(root.used(), base);
        let mut admitted = false;
        assert!(matches!(
            evaluate_eps_program(&finite, limits, &root, || {
                admitted |= root.used() > base;
                admitted
            }),
            Err(EpsVmError::Cancelled)
        ));
        assert!(admitted);
        assert_eq!(root.used(), base);
        let result = evaluate_eps_program(&finite, limits, &root, || false).unwrap();
        assert!(result.values().is_empty());
        drop(result);
        assert_eq!(root.used(), base);
        drop((finite, zero));
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn definitions_procedures_aliases_and_conditionals_execute_native_values() {
        assert_eq!(
            run(b"/average {add 2 div} def 40 60 average"),
            vec![EpsValue::Number(EpsNumber::Real(50.))]
        );
        assert_eq!(
            run(b"/inc {1 add} def 4 inc /plus /add load def 2 plus"),
            vec![int(7)]
        );
        assert_eq!(
            run(b"/x 1 def /x 9 def x /add {sub} def 10 3 add"),
            vec![int(9), int(7)]
        );
        assert_eq!(
            run(b"true {7} {9} ifelse false {100} if 3 {1 add} repeat"),
            vec![int(10)]
        );
        assert_eq!(run(b"{ {8} exec } exec"), vec![int(8)]);
        assert_eq!(run(b"/p { {} } def p p eq"), vec![EpsValue::Boolean(true)]);
        assert_eq!(run(b"7 /exec load dup exec"), vec![int(7)]);
        assert_eq!(run(b"/true false def true {1} {2} ifelse"), vec![int(2)]);
    }
    #[test]
    fn stack_numeric_and_identity_operations_follow_object_semantics() {
        assert_eq!(run(b"1 2 3 3 1 roll"), vec![int(3), int(1), int(2)]);
        assert_eq!(run(b"1 2 3 3 -1 roll"), vec![int(2), int(3), int(1)]);
        assert_eq!(
            run(b"1 2 2 copy 1 index count"),
            vec![int(1), int(2), int(1), int(2), int(1), int(5)]
        );
        assert_eq!(
            run(b"2147483647 1 add -2147483648 neg -2147483648 abs"),
            vec![EpsValue::Number(EpsNumber::Real(2147483648.)); 3]
        );
        assert_eq!(
            run(b"10 4 sub 3 mul 2 div"),
            vec![EpsValue::Number(EpsNumber::Real(9.))]
        );
        assert_eq!(
            run(b"{} dup eq {} {} eq 1 1.0 eq 2147483647 2147483648.0 eq"),
            vec![
                EpsValue::Boolean(true),
                EpsValue::Boolean(true),
                EpsValue::Boolean(true),
                EpsValue::Boolean(true)
            ]
        );
        assert_eq!(
            run(b"true false or 16#FF 16#0F and 0 not"),
            vec![EpsValue::Boolean(true), int(15), int(-1)]
        );
    }
    #[test]
    fn integer_division_remainder_and_square_root_obey_numeric_contract() {
        assert_eq!(run(b"5 2 idiv -5 2 idiv 5 -2 idiv -5 -2 idiv 5 3 mod -5 3 mod 5 -3 mod -5 -3 mod -2147483648 -1 mod"),
            [2,-2,-2,2,2,-2,2,-2,0].map(int).to_vec());
        assert_eq!(
            run(b"0 sqrt 9 sqrt 16.0 sqrt"),
            [0., 3., 4.]
                .map(|n| EpsValue::Number(EpsNumber::Real(n)))
                .to_vec()
        );
        let root = MemoryBudget::new(1_000_000);
        for (code, kind) in [
            (b"1 0 idiv".as_slice(), 0),
            (b"1 0 mod", 0),
            (b"-2147483648 -1 idiv", 0),
            (b"1.0 2 idiv", 1),
            (b"1 2.0 mod", 1),
            (b"true sqrt", 1),
            (b"-1 sqrt", 2),
        ] {
            let program = compile(code, &root);
            let error = evaluate_eps_program(&program, EpsVmLimits::default(), &root, || false).unwrap_err();
            assert!(
                match kind {
                    0 => matches!(error, EpsVmError::UndefinedResult),
                    1 => matches!(error, EpsVmError::Type),
                    _ => matches!(error, EpsVmError::Range),
                },
                "{code:?}: {error}"
            );
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn degree_trigonometry_preserves_cardinals_and_bounds_large_angles() {
        assert_eq!(
            run(b"0 sin 90 sin 180 sin 270 sin -90 sin 360090 sin 0 cos 90 cos 180 cos 270 cos"),
            [0., 1., 0., -1., -1., 1., 1., 0., -1., 0.]
                .map(|n| EpsValue::Number(EpsNumber::Real(n)))
                .to_vec()
        );
        for code in [b"3.4e38 sin".as_slice(), b"-3.4e38 cos"] {
            let result = run(code);
            let EpsValue::Number(EpsNumber::Real(n)) = result[0] else {
                panic!()
            };
            assert!(n.is_finite() && (-1. ..=1.).contains(&n));
        }
    }
    #[test]
    fn tiny_negative_degree_angles_do_not_disappear_during_reduction() {
        for (code, angle) in [
            (b"-1e-30 sin".as_slice(), -1e-30f32),
            (b"1e-30 sin", 1e-30f32),
            (b"-0.000001 sin", -0.000001f32),
        ] {
            let actual = run(code);
            let EpsValue::Number(EpsNumber::Real(value)) = actual[0] else {
                panic!()
            };
            // Independent small-angle expansion: cubic correction is below
            // f32 precision throughout this range.
            let expected = (f64::from(angle) * std::f64::consts::PI / 180.) as f32;
            assert_ne!(value, 0.);
            assert_eq!(value.to_bits(), expected.to_bits());
        }
        assert_eq!(
            run(b"-180 sin -270 sin -360 sin -180 cos -270 cos -360 cos"),
            [0., 1., 0., -1., 0., 1.]
                .map(|n| EpsValue::Number(EpsNumber::Real(n)))
                .to_vec()
        );
    }
    #[test]
    fn degree_trigonometry_matches_exact_binary_ghostscript_reference() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/trig-binary-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 2894);
        let ordered = |bits: u32| {
            if bits & 0x80000000 != 0 {
                !bits
            } else {
                bits | 0x80000000
            }
        };
        let mut maximum = 0;
        for case in cases {
            let code = case["code"].as_str().unwrap();
            let actual = run(code.as_bytes());
            let EpsValue::Number(EpsNumber::Real(value)) = actual[0] else {
                panic!()
            };
            let expected = case["real_bits"].as_u64().unwrap() as u32;
            let distance = ordered(value.to_bits()).abs_diff(ordered(expected));
            maximum = maximum.max(distance);
            assert!(
                distance == 0,
                "{code}: native={value:?}, reference={:?}, ULP={distance}",
                f32::from_bits(expected)
            );
        }
        eprintln!(
            "EPS sin/cos binary reference: {} cases, maximum {maximum} ULP",
            cases.len()
        );
    }
    #[test]
    fn logarithms_match_binary_oracle_and_reject_invalid_domains() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/log-binary-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 496);
        for case in cases {
            let code = case["code"].as_str().unwrap();
            let actual = run(code.as_bytes());
            let EpsValue::Number(EpsNumber::Real(value)) = actual[0] else {
                panic!()
            };
            assert_eq!(
                value.to_bits(),
                case["real_bits"].as_u64().unwrap() as u32,
                "{code}"
            );
        }
        let root = MemoryBudget::new(1_000_000);
        for code in [
            b"0 ln".as_slice(),
            b"-1 ln",
            b"0 log",
            b"-0.0 log",
            b"-1 log",
            b"true log",
            b"ln",
        ] {
            let program = compile(code, &root);
            let error = evaluate_eps_program(&program, EpsVmLimits::default(), &root, || false).unwrap_err();
            assert!(
                if code == b"true log" {
                    matches!(error, EpsVmError::Type)
                } else if code == b"ln" {
                    matches!(error, EpsVmError::StackUnderflow)
                } else {
                    matches!(error, EpsVmError::Range)
                },
                "{code:?}: {error}"
            );
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn arctangent_quadrants_match_binary_oracle_and_refuse_zero_vector() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/atan-binary-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 1094);
        for case in cases {
            let code = case["code"].as_str().unwrap();
            let actual = run(code.as_bytes());
            let EpsValue::Number(EpsNumber::Real(value)) = actual[0] else {
                panic!()
            };
            assert_eq!(
                value.to_bits(),
                case["real_bits"].as_u64().unwrap() as u32,
                "{code}"
            );
        }
        let root = MemoryBudget::new(1_000_000);
        for code in [
            b"0 0 atan".as_slice(),
            b"-0.0 0 atan",
            b"true 1 atan",
            b"1 true atan",
            b"1 atan",
        ] {
            let program = compile(code, &root);
            let error = evaluate_eps_program(&program, EpsVmLimits::default(), &root, || false).unwrap_err();
            assert!(
                if code == b"1 atan" {
                    matches!(error, EpsVmError::StackUnderflow)
                } else if code.starts_with(b"true") || code == b"1 true atan" {
                    matches!(error, EpsVmError::Type)
                } else {
                    matches!(error, EpsVmError::UndefinedResult)
                },
                "{code:?}: {error}"
            );
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn powers_match_binary_oracle_and_refuse_nonfinite_results() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/exp-binary-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 66);
        for case in cases {
            let code = case["code"].as_str().unwrap();
            let actual = run(code.as_bytes());
            let EpsValue::Number(EpsNumber::Real(value)) = actual[0] else {
                panic!()
            };
            assert_eq!(
                value.to_bits(),
                case["real_bits"].as_u64().unwrap() as u32,
                "{code}"
            );
        }
        let root = MemoryBudget::new(1_000_000);
        for code in [
            b"-2 0.5 exp".as_slice(),
            b"0 -1 exp",
            b"10 40 exp",
            b"true 2 exp",
            b"2 true exp",
            b"2 exp",
        ] {
            let program = compile(code, &root);
            let error = evaluate_eps_program(&program, EpsVmLimits::default(), &root, || false).unwrap_err();
            assert!(
                if code == b"2 exp" {
                    matches!(error, EpsVmError::StackUnderflow)
                } else if code.starts_with(b"true") || code == b"2 true exp" {
                    matches!(error, EpsVmError::Type)
                } else {
                    matches!(error, EpsVmError::UndefinedResult)
                },
                "{code:?}: {error}"
            );
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn numeric_conversions_preserve_types_and_check_integer_range() {
        assert_eq!(run(b"2147483647 cvi -2147483648 cvi 2147483520.0 cvi -2147483648.0 cvi -47.8 cvi 0.9 cvi -0.9 cvi"),
            [i32::MAX, i32::MIN, 2147483520, i32::MIN, -47, 0, 0].map(int).to_vec());
        assert_eq!(
            run(b"1 cvr 3.5 cvr 16777217 cvr"),
            [1., 3.5, 16777216.]
                .map(|n| EpsValue::Number(EpsNumber::Real(n)))
                .to_vec()
        );
        let root = MemoryBudget::new(1_000_000);
        for code in [
            b"2147483648.0 cvi".as_slice(),
            b"-2147483904.0 cvi",
            b"true cvi",
            b"true cvr",
            b"cvi",
            b"cvr",
        ] {
            let program = compile(code, &root);
            let error = evaluate_eps_program(&program, EpsVmLimits::default(), &root, || false).unwrap_err();
            assert!(
                if code.starts_with(b"true") {
                    matches!(error, EpsVmError::Type)
                } else if code == b"cvi" || code == b"cvr" {
                    matches!(error, EpsVmError::StackUnderflow)
                } else {
                    matches!(error, EpsVmError::Range)
                },
                "{code:?}: {error}"
            );
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn rounding_preserves_types_and_uses_positive_ties() {
        assert_eq!(
            run(b"99 ceiling -99 floor 99 round -99 truncate"),
            vec![int(99), int(-99), int(99), int(-99)]
        );
        let expected = [4., -4., 3., -5., 7., -6., 3., -3.];
        assert_eq!(run(b"3.2 ceiling -4.8 ceiling 3.2 floor -4.8 floor 6.5 round -6.5 round 3.9 truncate -3.9 truncate"),
            expected.map(|n| EpsValue::Number(EpsNumber::Real(n))).to_vec());
        // The immediately adjacent f32 values must not turn into a tie during
        // intermediate arithmetic, even around a representable integer.
        assert_eq!(
            run(b"0.49999997 round -0.50000006 round 8388609.0 round"),
            [0., -1., 8388609.]
                .map(|n| EpsValue::Number(EpsNumber::Real(n)))
                .to_vec()
        );
    }
    #[test]
    fn rounding_errors_release_managed_execution_memory() {
        let root = MemoryBudget::new(1_000_000);
        for code in [b"round".as_slice(), b"true floor", b"/x ceiling", b"{} truncate"] {
            let program = compile(code, &root);
            assert!(matches!(
                evaluate_eps_program(&program, EpsVmLimits::default(), &root, || false),
                Err(EpsVmError::StackUnderflow | EpsVmError::Type)
            ));
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn independent_ghostscript_values_and_types_match_scalar_and_procedure_cases() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/vm-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 35);
        for case in cases {
            let code = case["code"].as_str().unwrap();
            let actual = run(code.as_bytes());
            let expected = case["values"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{code}");
            for (actual, expected) in actual.iter().zip(expected) {
                match expected["type"].as_str().unwrap() {
                    "integer" => assert_eq!(
                        *actual,
                        int(i32::try_from(expected["value"].as_i64().unwrap()).unwrap()),
                        "{code}"
                    ),
                    "real" => assert_eq!(
                        *actual,
                        EpsValue::Number(EpsNumber::Real(expected["value"].as_f64().unwrap() as f32)),
                        "{code}"
                    ),
                    "boolean" => assert_eq!(
                        *actual,
                        EpsValue::Boolean(expected["value"].as_bool().unwrap()),
                        "{code}"
                    ),
                    "literal_name" => assert_eq!(
                        *actual,
                        EpsValue::LiteralName(expected["value"].as_str().unwrap().as_bytes()),
                        "{code}"
                    ),
                    _ => panic!("unknown reference type"),
                }
            }
        }
    }
    #[test]
    fn operator_exec_chains_do_not_recurse_on_the_rust_stack() {
        let mut code = String::from("7 ");
        code.push_str(&"/exec load ".repeat(3000));
        code.push_str("exec");
        let root = MemoryBudget::new(1_000_000);
        let program = compile(code.as_bytes(), &root);
        let evaluation = evaluate_eps_program(
            &program,
            EpsVmLimits {
                max_execution_frames: 1,
                ..EpsVmLimits::default()
            },
            &root,
            || false,
        )
        .unwrap();
        assert_eq!(evaluation.values(), &[int(7)]);
        drop(evaluation);
        drop(program);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn failures_limits_cancel_and_last_owner_release_all_managed_storage() {
        let limits = EpsVmLimits {
            max_operands: 8,
            max_dictionary_entries: 8,
            max_execution_frames: 4,
            max_work: 1000,
        };
        let root = MemoryBudget::new(1_000_000);
        for bytes in [
            b"pop".as_slice(),
            b"1 true add",
            b"1 0 div",
            b"(text)",
            b"]",
            b"not_defined",
            b"/x 1 def { //x }",
            b"1 -1 index",
            b"1 1e38 mul 1e38 mul",
        ] {
            let program = compile(bytes, &root);
            let baseline = root.used();
            assert!(
                evaluate_eps_program(&program, limits, &root, || false).is_err(),
                "{bytes:?}"
            );
            assert_eq!(root.used(), baseline);
            drop(program);
            assert_eq!(root.used(), 0);
        }
        let recursive = compile(b"/r {r} def r", &root);
        let baseline = root.used();
        assert!(matches!(
            evaluate_eps_program(&recursive, limits, &root, || false),
            Err(EpsVmError::Limit(EpsVmLimit::ExecutionFrames))
        ));
        assert_eq!(root.used(), baseline);
        let mut polls = 0;
        assert!(matches!(
            evaluate_eps_program(&recursive, limits, &root, || {
                polls += 1;
                polls == 7
            }),
            Err(EpsVmError::Cancelled)
        ));
        assert_eq!(polls, 7);
        assert_eq!(root.used(), baseline);
        drop(recursive);
        let finite = compile(b"1 1 add", &root);
        let baseline = root.used();
        assert!(matches!(
            evaluate_eps_program(
                &finite,
                EpsVmLimits {
                    max_work: 1,
                    ..limits
                },
                &root,
                || false
            ),
            Err(EpsVmError::Limit(EpsVmLimit::Work))
        ));
        assert!(matches!(
            evaluate_eps_program(
                &finite,
                EpsVmLimits {
                    max_operands: 1,
                    ..limits
                },
                &root,
                || false
            ),
            Err(EpsVmError::Limit(EpsVmLimit::Operands))
        ));
        assert!(matches!(
            evaluate_eps_program(&finite, limits, &root.child(1), || false),
            Err(EpsVmError::Memory(_))
        ));
        assert_eq!(root.used(), baseline);
        let values = evaluate_eps_program(&finite, limits, &root, || false).unwrap();
        let retained = values.clone();
        drop(values);
        drop(finite);
        assert_eq!(
            root.used(),
            (limits.max_operands * std::mem::size_of::<EpsValue<'_>>()) as u64
        );
        drop(retained);
        assert_eq!(root.used(), 0);
        let definitions = compile(b"/x 1 def /y 2 def", &root);
        assert!(matches!(
            evaluate_eps_program(
                &definitions,
                EpsVmLimits {
                    max_dictionary_entries: 1,
                    ..limits
                },
                &root,
                || false
            ),
            Err(EpsVmError::Limit(EpsVmLimit::Dictionary))
        ));
        drop(definitions);
        assert_eq!(root.used(), 0);
    }
}

#[cfg(test)]
mod vector_tests {
    use super::*;
    use crate::{EpsCompileLimits, EpsGraphicsLimit, EpsPathSegment, EpsSource, compile_eps_program};
    fn vm() -> EpsVmLimits {
        EpsVmLimits {
            max_operands: 128,
            max_dictionary_entries: 32,
            max_execution_frames: 16,
            max_work: 100_000,
        }
    }
    #[test]
    fn native_dictionaries_match_independent_ghostscript_traces() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/dictionary-ghostscript-reference.json"
        ))
        .unwrap();
        assert_eq!(reference["cases"].as_array().unwrap().len(), 20);
        for case in reference["cases"].as_array().unwrap() {
            let root = MemoryBudget::new(1_000_000);
            let source = case["code"].as_str().unwrap().as_bytes();
            let program = compile(source, &root);
            let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false);
            if let Some(error) = case.get("error") {
                assert!(
                    match error.as_str().unwrap() {
                        "invalidrestore" => matches!(result, Err(EpsVmError::InvalidRestore)),
                        "dictstackunderflow" => matches!(result, Err(EpsVmError::DictionaryStackUnderflow)),
                        "undefined" => matches!(result, Err(EpsVmError::UndefinedName(_))),
                        other => panic!("unexpected reference error {other}"),
                    },
                    "{}: {result:?}",
                    String::from_utf8_lossy(source)
                );
            } else {
                let result = result.unwrap_or_else(|e| panic!("{}: {e}", String::from_utf8_lossy(source)));
                let expected = case["values"].as_array().unwrap();
                assert_eq!(result.evaluation.values().len(), expected.len());
                for (actual, expected) in result.evaluation.values().iter().zip(expected) {
                    let actual = match actual {
                        EpsValue::Number(n) => as_double(*n),
                        EpsValue::Boolean(b) => {
                            if *b {
                                1.
                            } else {
                                0.
                            }
                        }
                        other => panic!("unexpected {other:?}"),
                    };
                    assert_eq!(
                        actual,
                        expected.as_f64().unwrap(),
                        "{}",
                        String::from_utf8_lossy(source)
                    );
                }
                drop(result);
            }
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn native_dictionary_cancellation_work_quota_and_limits_release_credit() {
        let root = MemoryBudget::new(1_000_000);
        let source = b"/d 0 dict def d begin /x 1 def /p {x} def end save /s exch def d /x 9 put d begin x end s restore d /x get";
        let program = compile(source, &root);
        let baseline = root.used();
        let mut polls = 0;
        drop(
            evaluate_eps_vectors(&program, vm(), graphics(), &root, || {
                polls += 1;
                false
            })
            .unwrap(),
        );
        for stop in 1..=polls {
            let mut at = 0;
            assert!(
                matches!(
                    evaluate_eps_vectors(&program, vm(), graphics(), &root, || {
                        at += 1;
                        at == stop
                    }),
                    Err(EpsVmError::Cancelled)
                ),
                "cancel point {stop}"
            );
            assert_eq!(root.used(), baseline);
        }
        let mut succeeded = false;
        for allowance in 0..10_000 {
            let mut limits = vm();
            limits.max_work = allowance;
            match evaluate_eps_vectors(&program, limits, graphics(), &root, || false) {
                Err(EpsVmError::Limit(EpsVmLimit::Work)) => {}
                Ok(result) => {
                    drop(result);
                    succeeded = true;
                    break;
                }
                other => panic!("work limit {allowance}: {other:?}"),
            }
            assert_eq!(root.used(), baseline);
        }
        assert!(succeeded);
        let (mut low, mut high) = (0, 100_000);
        while low < high {
            let middle = (low + high) / 2;
            let child = root.child(middle);
            match evaluate_eps_vectors(&program, vm(), graphics(), &child, || false) {
                Ok(result) => {
                    drop(result);
                    high = middle;
                }
                Err(EpsVmError::Memory(_) | EpsVmError::Graphics(EpsGraphicsError::Memory(_))) => {
                    low = middle + 1
                }
                other => panic!("quota {middle}: {other:?}"),
            }
            assert_eq!(child.used(), 0);
        }
        drop(evaluate_eps_vectors(&program, vm(), graphics(), &root.child(low), || false).unwrap());
        assert!(evaluate_eps_vectors(&program, vm(), graphics(), &root.child(low - 1), || false).is_err());
        assert_eq!(root.used(), baseline);
        drop(program);
        for (code, limit) in [
            (
                b"/d 1 dict def 20 {d begin} repeat".as_slice(),
                EpsVmLimit::DictionaryStack,
            ),
            (b"40 {0 dict pop} repeat", EpsVmLimit::Dictionary),
        ] {
            let program = compile(code, &root);
            assert!(
                matches!(evaluate_eps_program(&program, vm(), &root, || false), Err(EpsVmError::Limit(actual)) if actual==limit)
            );
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn native_dictionaries_scope_alias_growth_and_rollback() {
        for (source, expected) in [
            (b"/x 1 def /d 1 dict def d begin /x 2 def x currentdict d eq end x".as_slice(), vec![EpsValue::Number(EpsNumber::Integer(2)), EpsValue::Boolean(true), EpsValue::Number(EpsNumber::Integer(1))]),
            (b"0 dict dup /x 3 put dup /y 4 put dup length exch /x get", vec![EpsValue::Number(EpsNumber::Integer(2)), EpsValue::Number(EpsNumber::Integer(3))]),
            (b"/d 1 dict def /alias d def d /x 1 put save /s exch def alias /x 2 put alias /y 3 put s restore d /x get d /y known", vec![EpsValue::Number(EpsNumber::Integer(1)), EpsValue::Boolean(false)]),
            (b"/d 1 dict def d begin /x 1 def save end restore d /x get", vec![EpsValue::Number(EpsNumber::Integer(1))]),
            (b"currentdict /x 9 put x currentdict /x known", vec![EpsValue::Number(EpsNumber::Integer(9)), EpsValue::Boolean(true)]),
        ] {
            let root = MemoryBudget::new(1_000_000);
            let program = compile(source, &root);
            let result = evaluate_eps_program(&program, vm(), &root, || false).unwrap();
            assert_eq!(result.values(), expected, "{}", String::from_utf8_lossy(source));
            drop((result,program)); assert_eq!(root.used(),0);
        }
    }
    #[test]
    fn native_dictionary_restore_rejects_discarded_operand_and_stack_references() {
        for source in [
            b"save /s exch def 0 dict s restore".as_slice(),
            b"save /s exch def 1 dict begin s restore",
        ] {
            let root = MemoryBudget::new(1_000_000);
            let program = compile(source, &root);
            assert!(matches!(
                evaluate_eps_program(&program, vm(), &root, || false),
                Err(EpsVmError::InvalidRestore)
            ));
            drop(program);
            assert_eq!(root.used(), 0);
        }
        let root = MemoryBudget::new(1_000_000);
        let program = compile(b"end", &root);
        assert!(matches!(
            evaluate_eps_program(&program, vm(), &root, || false),
            Err(EpsVmError::DictionaryStackUnderflow)
        ));
    }
    #[test]
    fn vm_save_restore_cancellation_preserves_live_machine_and_graphics() {
        let root = MemoryBudget::new(100_000);
        let mut g = EpsGraphics::new(graphics(), &root, || false).unwrap();
        g.set_line_width(2.).unwrap();
        let mut machine = Machine {
            tape: &[],
            saves: None,
            save_depth: 0,
            next_save: 1,
            dict_stack: root.try_buffer(0, EpsValue::Null).unwrap(),
            dict_depth: 0,
            retain_user_dictionary: false,
            bindings: root.try_buffer(0, Binding::default()).unwrap(),
            graphics: Some(&mut g),
            operands: root.try_buffer(8, EpsValue::Null).unwrap(),
            arrays: root.try_buffer(4, EpsValue::Null).unwrap(),
            array_used: 2,
            array_capacity: 4,
            array_budget: root.clone(),
            length: 1,
            dictionary: root
                .try_buffer(
                    4,
                    Entry {
                        key: None,
                        value: EpsValue::Null,
                    },
                )
                .unwrap(),
            dict_records: root.try_buffer(0, DictRecord::default()).unwrap(),
            dict_used: 0,
            frames: root
                .try_buffer(
                    4,
                    Frame {
                        begin: 0,
                        pc: 0,
                        end: 0,
                        continuation: Continuation::Once,
                    },
                )
                .unwrap(),
            depth: 1,
        };
        machine.operands[0] = EpsValue::Number(EpsNumber::Integer(7));
        machine.arrays[1] = EpsValue::Number(EpsNumber::Integer(2));
        machine.dictionary[0] = Entry {
            key: Some(b"x"),
            value: EpsValue::Number(EpsNumber::Integer(1)),
        };
        let baseline = root.used();
        let mut capture_polls = 0;
        machine
            .save_vm(&mut Work {
                remaining: 10000,
                cancelled: || {
                    capture_polls += 1;
                    false
                },
            })
            .unwrap();
        machine
            .restore_vm(&mut Work {
                remaining: 10000,
                cancelled: || false,
            })
            .unwrap();
        assert_eq!(root.used(), baseline);
        for stop in 1..=capture_polls {
            let mut at = 0;
            assert!(matches!(
                machine.save_vm(&mut Work {
                    remaining: 10000,
                    cancelled: || {
                        at += 1;
                        at == stop
                    }
                }),
                Err(EpsVmError::Cancelled)
            ));
            assert_eq!(machine.length, 1);
            assert!(machine.saves.is_none());
            assert_eq!(machine.save_depth, 0);
            assert_eq!(machine.next_save, 2);
            assert_eq!(machine.graphics.as_ref().unwrap().style().width, 2.);
            assert_eq!(root.used(), baseline);
        }
        machine
            .save_vm(&mut Work {
                remaining: 10000,
                cancelled: || false,
            })
            .unwrap();
        machine.arrays[1] = EpsValue::Number(EpsNumber::Integer(9));
        machine.dictionary[0].value = EpsValue::Number(EpsNumber::Integer(8));
        machine.graphics.as_mut().unwrap().set_line_width(6.).unwrap();
        machine.graphics.as_mut().unwrap().gsave().unwrap();
        machine.graphics.as_mut().unwrap().set_line_width(9.).unwrap();
        let charged = root.used();
        // Restore has three VM checkpoints, followed by the empty graphics stack's
        // initial/final validation checkpoints. No cancellation can half-restore.
        for stop in 1..=5 {
            let mut at = 0;
            assert!(matches!(
                machine.restore_vm(&mut Work {
                    remaining: 10000,
                    cancelled: || {
                        at += 1;
                        at == stop
                    }
                }),
                Err(EpsVmError::Cancelled)
            ));
            assert_eq!(machine.length, 2);
            assert_eq!(machine.save_depth, 1);
            assert_eq!(machine.arrays[1], EpsValue::Number(EpsNumber::Integer(9)));
            assert_eq!(
                machine.dictionary[0].value,
                EpsValue::Number(EpsNumber::Integer(8))
            );
            assert_eq!(machine.graphics.as_ref().unwrap().style().width, 9.);
            assert_eq!(root.used(), charged);
        }
        machine
            .restore_vm(&mut Work {
                remaining: 10000,
                cancelled: || false,
            })
            .unwrap();
        assert_eq!(machine.arrays[1], EpsValue::Number(EpsNumber::Integer(2)));
        assert_eq!(
            machine.dictionary[0].value,
            EpsValue::Number(EpsNumber::Integer(1))
        );
        assert_eq!(machine.graphics.as_ref().unwrap().style().width, 2.);
        assert_eq!(root.used(), baseline);
        drop(machine);
        drop(g);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn vm_save_restore_every_cancellation_and_short_quota_releases_credit() {
        let root = MemoryBudget::new(1_000_000);
        let program = compile(b"/a [1 2] def /p {1 2 add} def /p load bind pop 2 setlinewidth save /s exch def a 0 9 put 3 setlinewidth gsave 4 setlinewidth 0 0 1 1 rectfill s restore a 0 get p currentlinewidth", &root);
        let baseline = root.used();
        let mut polls = 0;
        let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || {
            polls += 1;
            false
        })
        .unwrap();
        drop(result);
        assert_eq!(root.used(), baseline);
        for stop in 1..=polls {
            let mut at = 0;
            let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || {
                at += 1;
                at == stop
            });
            assert!(
                matches!(result, Err(EpsVmError::Cancelled)),
                "cancel checkpoint {stop}/{polls}: {result:?}"
            );
            assert_eq!(root.used(), baseline);
        }
        let mut work_succeeded = false;
        for allowance in 0..10_000 {
            let mut limits = vm();
            limits.max_work = allowance;
            match evaluate_eps_vectors(&program, limits, graphics(), &root, || false) {
                Err(EpsVmError::Limit(EpsVmLimit::Work)) => {}
                Ok(result) => {
                    drop(result);
                    work_succeeded = true;
                    break;
                }
                other => panic!("work allowance {allowance}: {other:?}"),
            }
            assert_eq!(root.used(), baseline);
        }
        assert!(work_succeeded);
        // Binary search the exact admitted payload boundary for the full program.
        let mut low = 0;
        let mut high = 100_000;
        while low < high {
            let middle = (low + high) / 2;
            let child = root.child(middle);
            match evaluate_eps_vectors(&program, vm(), graphics(), &child, || false) {
                Ok(result) => {
                    drop(result);
                    high = middle;
                }
                Err(EpsVmError::Memory(_) | EpsVmError::Graphics(EpsGraphicsError::Memory(_))) => {
                    low = middle + 1
                }
                other => panic!("quota {middle}: {other:?}"),
            }
            assert_eq!(child.used(), 0);
            assert_eq!(root.used(), baseline);
        }
        assert!(low > 0);
        let exact = root.child(low);
        drop(evaluate_eps_vectors(&program, vm(), graphics(), &exact, || false).unwrap());
        assert_eq!(exact.used(), 0);
        let short = root.child(low - 1);
        assert!(evaluate_eps_vectors(&program, vm(), graphics(), &short, || false).is_err());
        assert_eq!(short.used(), 0);
        drop(program);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn vm_save_restore_matches_independent_ghostscript_traces() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/save-ghostscript-reference.json"
        ))
        .unwrap();
        assert_eq!(reference["cases"].as_array().unwrap().len(), 30);
        for case in reference["cases"].as_array().unwrap() {
            let root = MemoryBudget::new(1_000_000);
            let source = case["code"].as_str().unwrap().as_bytes();
            let program = compile(source, &root);
            let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false);
            if case.get("error").is_some() {
                assert_eq!(case["error"], "invalidrestore");
                assert!(
                    matches!(result, Err(EpsVmError::InvalidRestore)),
                    "{}: {result:?}",
                    String::from_utf8_lossy(source)
                );
            } else {
                let result = result.unwrap_or_else(|e| panic!("{}: {e}", String::from_utf8_lossy(source)));
                let expected = case["values"].as_array().unwrap();
                assert_eq!(
                    result.evaluation.values().len(),
                    expected.len(),
                    "{}",
                    String::from_utf8_lossy(source)
                );
                for (actual, expected) in result.evaluation.values().iter().zip(expected) {
                    let EpsValue::Number(actual) = actual else {
                        panic!("unexpected {actual:?}");
                    };
                    assert_eq!(as_double(*actual), expected.as_f64().unwrap());
                }
                drop(result);
            }
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn vm_save_restore_rolls_back_supported_objects_and_preserves_operands() {
        for (source, expected) in [
            (
                b"/x 1 def /a [2 3] def save /s exch def /x 9 def a 0 8 put s restore x a 0 get a 1 get"
                    .as_slice(),
                vec![1, 2, 3],
            ),
            (b"7 save 8 exch restore 9", vec![7, 8, 9]),
            (
                b"/x 1 def save /s exch def /x 2 def save /t exch def /x 3 def t restore x s restore x",
                vec![2, 1],
            ),
            (b"save /s exch def [1] pop s restore 4", vec![4]),
            (b"save /s exch def save /t exch def s restore 5", vec![5]),
            (b"/f { s restore } def save /s exch def f 7", vec![7]),
            (b"/f { {} } def save /s exch def f s restore pop 8", vec![8]),
            (
                b"/a [1] def a save /s exch def /a [9] def s restore 0 get",
                vec![1],
            ),
            (
                b"/a [1] def /p { a 0 2 put } def save /s exch def p s restore a 0 get",
                vec![1],
            ),
        ] {
            let root = MemoryBudget::new(1_000_000);
            let program = compile(source, &root);
            let result = evaluate_eps_program(&program, vm(), &root, || false).unwrap();
            let expected: Vec<_> = expected
                .into_iter()
                .map(|n| EpsValue::Number(EpsNumber::Integer(n)))
                .collect();
            assert_eq!(result.values(), expected, "{}", String::from_utf8_lossy(source));
            drop((result, program));
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn vm_restore_rejects_discarded_live_references_and_save_objects() {
        for source in [
            b"save /s exch def [1] s restore".as_slice(),
            b"save /s exch def {1} s restore",
            b"save dup restore restore",
            b"save /s exch def save s restore",
            b"save /s exch def {s restore 8} exec",
            b"/f {s restore} def save /s exch def /f {s restore 8} def f",
        ] {
            let root = MemoryBudget::new(1_000_000);
            let program = compile(source, &root);
            assert!(
                matches!(
                    evaluate_eps_program(&program, vm(), &root, || false),
                    Err(EpsVmError::InvalidRestore)
                ),
                "{}",
                String::from_utf8_lossy(source)
            );
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn vm_save_restore_restores_bindings_graphics_and_retains_paint() {
        let root = MemoryBudget::new(1_000_000);
        let source = b"/p {1 2 add} def 2 setlinewidth save /s exch def /p load bind pop /add {pop pop 99} def p 3 setlinewidth 0 0 1 1 rectfill s restore /add {pop pop 77} def p currentlinewidth 2 0 1 1 rectfill";
        let program = compile(source, &root);
        let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).unwrap();
        assert_eq!(
            result.evaluation.values(),
            &[
                EpsValue::Number(EpsNumber::Integer(3)),
                EpsValue::Number(EpsNumber::Integer(77)),
                EpsValue::Number(EpsNumber::Real(2.))
            ]
        );
        assert_eq!(result.scene.paints().len(), 2);
        assert_eq!(result.scene.paints()[0].style.width, 3.);
        assert_eq!(result.scene.paints()[1].style.width, 2.);
        drop((result, program));
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn grestoreall_vm_restores_bottom_and_preserves_typed_work_errors() {
        let root = MemoryBudget::new(1_000_000);
        let program = compile(b"2 setlinewidth gsave 3 setlinewidth gsave 4 setlinewidth grestoreall currentlinewidth 5 setlinewidth grestore currentlinewidth", &root);
        let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).unwrap();
        assert_eq!(
            result.evaluation.values(),
            &[
                EpsValue::Number(EpsNumber::Real(2.)),
                EpsValue::Number(EpsNumber::Real(5.))
            ]
        );
        drop(result);
        // Every sub-successful work allowance must retain Limit(Work), including
        // the scan inside grestoreall, instead of becoming Graphics(Cancelled).
        let mut succeeded = false;
        for allowance in 0..1000 {
            let mut limits = vm();
            limits.max_work = allowance;
            match evaluate_eps_vectors(&program, limits, graphics(), &root, || false) {
                Err(EpsVmError::Limit(EpsVmLimit::Work)) => {}
                Ok(result) => {
                    drop(result);
                    succeeded = true;
                    break;
                }
                other => panic!("unexpected work result at {allowance}: {other:?}"),
            }
        }
        assert!(succeeded);
        drop(program);
        assert_eq!(root.used(), 0);
    }
    fn graphics() -> EpsGraphicsLimits {
        EpsGraphicsLimits {
            max_nodes: 128,
            max_paints: 8,
            max_saved_states: 16,
        }
    }
    #[test]
    fn arc_operators_execute_and_account_each_generated_piece() {
        let root = MemoryBudget::new(100_000);
        for code in [
            b"0 0 2 0 360 arc currentpoint stroke".as_slice(),
            b"0 0 2 360 0 arcn currentpoint stroke".as_slice(),
        ] {
            let program = compile(code, &root);
            let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).unwrap();
            assert_eq!(result.scene.paints()[0].node_count, 5);
            assert_eq!(
                result.evaluation.values(),
                &[
                    EpsValue::Number(EpsNumber::Real(2.)),
                    EpsValue::Number(EpsNumber::Real(0.))
                ]
            );
            drop(result);
            drop(program);
            assert_eq!(root.used(), 0);
        }
        let program = compile(b"0 0 2 0 7200 arc stroke", &root);
        let before = root.used();
        let limited = EpsVmLimits { max_work: 20, ..vm() };
        assert!(matches!(
            evaluate_eps_vectors(&program, limited, graphics(), &root, || false),
            Err(EpsVmError::Limit(EpsVmLimit::Work))
        ));
        assert_eq!(root.used(), before);
        drop(program);
        assert_eq!(root.used(), 0);
    }
    fn compile<'a>(code: &'a [u8], root: &MemoryBudget) -> EpsProgram<'a> {
        compile_eps_program(
            &EpsSource {
                postscript: code,
                wmf_preview: None,
                tiff_preview: None,
            },
            EpsCompileLimits::default(),
            root,
            || false,
        )
        .unwrap()
    }
    #[test]
    fn native_vm_graphics_queries_match_all_independent_graphics_cases() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/graphics-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 17);
        for case in cases {
            let mut code = String::new();
            for action in case["actions"].as_array().unwrap() {
                let action = action.as_array().unwrap();
                for arg in &action[1..] {
                    code.push_str(&arg.to_string());
                    code.push(' ');
                }
                code.push_str(action[0].as_str().unwrap());
                code.push('\n');
            }
            let root = MemoryBudget::new(1_000_000);
            let program = compile(code.as_bytes(), &root);
            let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).unwrap();
            let expected = case["values"].as_array().unwrap();
            assert_eq!(result.evaluation.values().len(), expected.len());
            for (&value, expected) in result.evaluation.values().iter().zip(expected) {
                let EpsValue::Number(number) = value else { panic!() };
                assert!(
                    (as_double(number) - expected.as_f64().unwrap()).abs() <= 0.001,
                    "{case}: {value:?} vs {expected}"
                );
            }
            drop(result);
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn eps_import_showpage_is_empty_aliasable_and_user_redefinable() {
        let root = MemoryBudget::new(1_000_000);
        let program=compile(b"/showpage load {} eq 1 2 moveto showpage /flush /showpage load def flush currentpoint /showpage { 7 } def showpage",&root);
        let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).unwrap();
        assert_eq!(
            result.evaluation.values(),
            &[
                EpsValue::Boolean(true),
                EpsValue::Number(EpsNumber::Real(1.)),
                EpsValue::Number(EpsNumber::Real(2.)),
                EpsValue::Number(EpsNumber::Integer(7))
            ]
        );
        drop(result);
        drop(program);
        assert_eq!(root.used(), 0);
        let program = compile(b"showpage", &root);
        assert!(matches!(
            evaluate_eps_program(&program, vm(), &root, || false),
            Err(EpsVmError::UndefinedName(_))
        ));
        drop(program);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn numeric_rectangles_preserve_path_style_and_support_negative_sizes() {
        let root = MemoryBudget::new(1_000_000);
        let program=compile(b"1 2 moveto 3 4 lineto 2 setlinewidth 10 20 -4 -6 rectfill 1 1 2 3 rectstroke currentpoint stroke",&root);
        let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).unwrap();
        assert_eq!(
            result.evaluation.values(),
            &[
                EpsValue::Number(EpsNumber::Real(3.)),
                EpsValue::Number(EpsNumber::Real(4.))
            ]
        );
        assert_eq!(result.scene.paints().len(), 3);
        assert_eq!(result.scene.paints()[0].kind, EpsPaintKind::RectFill);
        assert_eq!(result.scene.paints()[1].kind, EpsPaintKind::Stroke);
        assert_eq!(result.scene.paints()[2].node_count, 2);
        assert!(result.scene.paints().iter().all(|p| p.style.width == 2.));
        let mut path = [EpsPathSegment::Move([0., 0.]); 5];
        result.scene.copy_path(0, &mut path, || false).unwrap();
        assert_eq!(path[0], EpsPathSegment::Move([10., 20.]));
        assert_eq!(
            path[2],
            EpsPathSegment::Line {
                start: [6., 20.],
                end: [6., 14.]
            }
        );
        drop(result);
        drop(program);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn array_concat_composes_in_order_and_rejects_invalid_matrices() {
        let root = MemoryBudget::new(1_000_000);
        let program = compile(
            b"2 3 translate [2 0 0 4 1 2] concat 1 1 moveto 2 2 lineto stroke",
            &root,
        );
        let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).unwrap();
        assert_eq!(result.scene.paints()[0].matrix, [2., 0., 0., 4., 3., 5.]);
        let mut path = [EpsPathSegment::Move([0., 0.]); 2];
        result.scene.copy_path(0, &mut path, || false).unwrap();
        assert_eq!(path[0], EpsPathSegment::Move([5., 9.]));
        assert_eq!(
            path[1],
            EpsPathSegment::Line {
                start: [5., 9.],
                end: [7., 13.]
            }
        );
        drop((result, program));
        assert_eq!(root.used(), 0);
        for source in [
            b"[] concat".as_slice(),
            b"[1 0 0 1 0 0 99] concat",
            b"[1 0 0 1 true 0] concat",
        ] {
            let program = compile(source, &root);
            let baseline = root.used();
            assert!(evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).is_err());
            assert_eq!(root.used(), baseline);
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }

    #[test]
    fn initgraphics_resets_defaults_without_destroying_saved_state() {
        let code = b"10 20 translate 6 7 moveto 4 setlinewidth 2 setlinecap 1 setlinejoin 5 setmiterlimit 0.75 setgray gsave initgraphics currentlinewidth currentlinecap currentlinejoin currentmiterlimit currentgray 0 0 moveto 1 1 lineto stroke grestore currentpoint currentlinewidth currentgray 8 9 lineto stroke";
        let root = MemoryBudget::new(1_000_000);
        let program = compile(code, &root);
        let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).unwrap();
        assert_eq!(
            result.evaluation.values(),
            &[
                EpsValue::Number(EpsNumber::Real(1.)),
                EpsValue::Number(EpsNumber::Integer(0)),
                EpsValue::Number(EpsNumber::Integer(0)),
                EpsValue::Number(EpsNumber::Real(10.)),
                EpsValue::Number(EpsNumber::Real(0.)),
                EpsValue::Number(EpsNumber::Real(6.)),
                EpsValue::Number(EpsNumber::Real(7.)),
                EpsValue::Number(EpsNumber::Real(4.)),
                EpsValue::Number(EpsNumber::Real(0.75)),
            ]
        );
        let paints = result.scene.paints();
        assert_eq!(paints.len(), 2);
        assert_eq!(paints[0].matrix, [1., 0., 0., 1., 0., 0.]);
        assert_eq!(paints[0].style, crate::EpsStrokeStyle::default());
        assert_eq!(paints[1].matrix, [1., 0., 0., 1., 10., 20.]);
        assert_eq!(paints[1].node_count, 2);
        assert_eq!(paints[1].style.width, 4.);
        assert_eq!(paints[1].style.color, EpsDeviceColor::Gray(0.75));
        drop(result);
        drop(program);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn initmatrix_preserves_built_path_style_and_saved_transform() {
        let code = b"7 8 moveto 3 setlinewidth 0.25 setgray 10 20 translate 2 3 scale gsave initmatrix currentpoint currentlinewidth currentgray 1 2 lineto stroke grestore 0 0 moveto 1 1 lineto stroke";
        let root = MemoryBudget::new(1_000_000);
        let program = compile(code, &root);
        let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).unwrap();
        assert_eq!(
            result.evaluation.values(),
            &[
                EpsValue::Number(EpsNumber::Real(7.)),
                EpsValue::Number(EpsNumber::Real(8.)),
                EpsValue::Number(EpsNumber::Real(3.)),
                EpsValue::Number(EpsNumber::Real(0.25)),
            ]
        );
        let paints = result.scene.paints();
        assert_eq!(paints.len(), 2);
        assert_eq!(paints[0].matrix, [1., 0., 0., 1., 0., 0.]);
        assert_eq!(paints[1].matrix, [2., 0., 0., 3., 10., 20.]);
        assert!(
            paints
                .iter()
                .all(|p| p.style.width == 3. && p.style.color == EpsDeviceColor::Gray(0.25))
        );
        let mut path = [EpsPathSegment::Move([0., 0.]); 3];
        assert_eq!(result.scene.copy_path(0, &mut path, || false).unwrap(), 2);
        assert_eq!(path[0], EpsPathSegment::Move([7., 8.]));
        assert_eq!(
            path[1],
            EpsPathSegment::Line {
                start: [7., 8.],
                end: [1., 2.]
            }
        );
        drop(result);
        drop(program);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn computed_procedures_operator_aliases_and_fill_stroke_build_one_shared_path() {
        let code=b"/M /moveto load def /box { M 10 0 rlineto 0 10 rlineto -10 0 rlineto closepath } def 1 1 add 9 3 div box 1 0 0 setrgbcolor gsave fill grestore 2 setlinewidth 2 3 scale stroke";
        let root = MemoryBudget::new(1_000_000);
        let program = compile(code, &root);
        let result = evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).unwrap();
        assert!(result.evaluation.values().is_empty());
        let paints = result.scene.paints();
        assert_eq!(paints.len(), 2);
        assert_eq!(paints[0].kind, EpsPaintKind::FillNonZero);
        assert_eq!(paints[1].kind, EpsPaintKind::Stroke);
        assert_eq!(paints[1].matrix, [2., 0., 0., 3., 0., 0.]);
        assert_eq!(paints[1].style.width, 2.);
        assert_eq!(paints[1].style.color, EpsDeviceColor::Rgb([1., 0., 0.]));
        let mut fill = [EpsPathSegment::Move([0., 0.]); 5];
        let mut stroke = fill;
        assert_eq!(result.scene.copy_path(0, &mut fill, || false).unwrap(), 5);
        result.scene.copy_path(1, &mut stroke, || false).unwrap();
        assert_eq!(fill, stroke);
        assert_eq!(fill[0], EpsPathSegment::Move([2., 3.]));
        assert_eq!(
            fill[4],
            EpsPathSegment::Close {
                start: [2., 13.],
                end: [2., 3.]
            }
        );
        let held = result.clone();
        drop(result);
        drop(program);
        assert!(root.used() > 0);
        drop(held);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn drawing_failure_cancellation_and_quotas_discard_the_entire_partial_scene() {
        let root = MemoryBudget::new(1_000_000);
        for code in [
            b"0 0 moveto 1 1 lineto stroke unknown".as_slice(),
            b"1 2 lineto",
            b"0 0 moveto 1 1 lineto stroke 1.0 setlinecap",
            b"0 0 moveto 1 1 lineto stroke clippath",
            b"0 0 0 0 setcmykcolor currentrgbcolor",
        ] {
            let program = compile(code, &root);
            let baseline = root.used();
            assert!(evaluate_eps_vectors(&program, vm(), graphics(), &root, || false).is_err());
            assert_eq!(root.used(), baseline);
            drop(program);
            assert_eq!(root.used(), 0);
        }
        let program = compile(b"0 0 moveto 1 1 lineto stroke", &root);
        let baseline = root.used();
        assert!(matches!(
            evaluate_eps_vectors(
                &program,
                vm(),
                EpsGraphicsLimits {
                    max_paints: 0,
                    ..graphics()
                },
                &root,
                || false
            ),
            Err(EpsVmError::Graphics(EpsGraphicsError::Limit(
                EpsGraphicsLimit::Paints
            )))
        ));
        assert_eq!(root.used(), baseline);
        assert!(evaluate_eps_vectors(&program, vm(), graphics(), &root.child(1), || false).is_err());
        assert_eq!(root.used(), baseline);
        let mut polls = 0;
        assert!(matches!(
            evaluate_eps_vectors(&program, vm(), graphics(), &root, || {
                polls += 1;
                polls == 20
            }),
            Err(EpsVmError::Cancelled)
        ));
        assert_eq!(polls, 20);
        assert_eq!(root.used(), baseline);
        assert!(matches!(
            evaluate_eps_program(&program, vm(), &root, || false),
            Err(EpsVmError::Unsupported(_))
        ));
        assert_eq!(root.used(), baseline);
        drop(program);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn path_bbox_preserves_original_vm_work_limit_and_transient_cancel_causes() {
        let root = MemoryBudget::new(100_000);
        let mut g = EpsGraphics::new(graphics(), &root, || false).unwrap();
        g.move_to(1., 2.).unwrap();
        g.line_to(3., 4.).unwrap();
        {
            let mut machine = Machine {
                tape: &[],
                saves: None,
                save_depth: 0,
                next_save: 1,
                dict_stack: root.try_buffer(0, EpsValue::Null).unwrap(),
                dict_depth: 0,
                retain_user_dictionary: false,
                bindings: root.try_buffer(0, Binding::default()).unwrap(),
                graphics: Some(&mut g),
                operands: root.try_buffer(8, EpsValue::Null).unwrap(),
                arrays: root.try_buffer(0, EpsValue::Null).unwrap(),
                array_used: 0,
                array_capacity: 0,
                array_budget: root.clone(),
                length: 0,
                dictionary: root
                    .try_buffer(
                        0,
                        Entry {
                            key: None,
                            value: EpsValue::Null,
                        },
                    )
                    .unwrap(),
                dict_records: root.try_buffer(0, DictRecord::default()).unwrap(),
                dict_used: 0,
                frames: root
                    .try_buffer(
                        0,
                        Frame {
                            begin: 0,
                            pc: 0,
                            end: 0,
                            continuation: Continuation::Once,
                        },
                    )
                    .unwrap(),
                depth: 0,
            };
            let mut work = Work {
                remaining: 0,
                cancelled: || false,
            };
            assert!(matches!(
                machine.graphics_operation(EpsOperator::PathBBox, 0, &mut work),
                Err(EpsVmError::Limit(EpsVmLimit::Work))
            ));
            let mut polls = 0;
            let mut work = Work {
                remaining: 100,
                cancelled: || {
                    polls += 1;
                    polls == 2
                },
            };
            assert!(matches!(
                machine.graphics_operation(EpsOperator::PathBBox, 0, &mut work),
                Err(EpsVmError::Cancelled)
            ));
            assert_eq!(polls, 2);
        }
        assert_eq!(g.current_point().unwrap(), [3., 4.]);
        drop(g);
        assert_eq!(root.used(), 0);
    }
}
