//! Budgeted borrowed instruction tape for a future native PostScript interpreter.
use crate::EpsStringError;
use crate::eps_strings::validate_eps_string;
use crate::{EpsNumber, EpsNumberError, EpsSource, EpsToken, EpsTokenError, parse_eps_number};
use rrrah_core::{BufferError, MemoryBudget, SharedBuffer};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy)]
pub struct EpsCompileLimits {
    pub max_instructions: usize,
    pub max_token_bytes: usize,
    pub max_decoded_string_bytes: usize,
    /// At most 128; the procedure index stack has fixed stack storage.
    pub max_procedure_depth: usize,
}
impl Default for EpsCompileLimits {
    fn default() -> Self {
        Self {
            max_instructions: 1_000_000,
            max_token_bytes: 64 * 1024,
            max_decoded_string_bytes: 256 * 1024,
            max_procedure_depth: 128,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EpsInstruction<'a> {
    pub token: EpsToken<'a>,
    /// Cached only for executable words. Literal/immediate names retain names.
    pub number: Option<EpsNumber>,
    /// Validated decoded byte length; payload still borrows its encoded source.
    pub decoded_string_bytes: Option<usize>,
    /// Matching brace instruction, for both opening and closing braces.
    pub procedure_partner: Option<usize>,
}
#[derive(Debug, Clone)]
pub struct EpsProgram<'a> {
    instructions: SharedBuffer<EpsInstruction<'a>>,
}
impl<'a> EpsProgram<'a> {
    pub fn instructions(&self) -> &[EpsInstruction<'a>] {
        &self.instructions
    }
}
#[derive(Debug, thiserror::Error)]
pub enum EpsCompileError {
    #[error("unmatched PostScript procedure brace at instruction {0}")]
    Procedure(usize),
    #[error("PostScript procedure nesting limit exceeded")]
    Limit,
    #[error("PostScript program compilation cancelled")]
    Cancelled,
    #[error(transparent)]
    Lexical(EpsTokenError),
    #[error(transparent)]
    Number(EpsNumberError),
    #[error(transparent)]
    String(EpsStringError),
    #[error(transparent)]
    Memory(#[from] BufferError),
}
fn walk<'a>(
    source: &EpsSource<'a>,
    limits: EpsCompileLimits,
    cancelled: &dyn Fn() -> bool,
    mut emit: impl FnMut(usize, EpsInstruction<'a>),
) -> Result<usize, EpsCompileError> {
    use EpsCompileError as E;
    if cancelled() {
        return Err(E::Cancelled);
    }
    if limits.max_procedure_depth > 128 {
        return Err(E::Limit);
    }
    let mut stack = [0usize; 128];
    let mut depth = 0;
    let mut count = 0;
    for token in source.tokens(limits.max_instructions, limits.max_token_bytes, cancelled) {
        let token = token.map_err(|e| match e {
            EpsTokenError::Cancelled => E::Cancelled,
            other => E::Lexical(other),
        })?;
        let mut instruction = EpsInstruction {
            token,
            number: None,
            decoded_string_bytes: None,
            procedure_partner: None,
        };
        if let EpsToken::Word(word) = token {
            instruction.number =
                parse_eps_number(word, limits.max_token_bytes, cancelled).map_err(|e| match e {
                    EpsNumberError::Cancelled => E::Cancelled,
                    other => E::Number(other),
                })?;
        }
        if matches!(
            token,
            EpsToken::String(_) | EpsToken::HexString(_) | EpsToken::Ascii85String(_)
        ) {
            instruction.decoded_string_bytes = Some(
                validate_eps_string(token, limits.max_decoded_string_bytes, cancelled).map_err(
                    |e| match e {
                        EpsStringError::Cancelled => E::Cancelled,
                        other => E::String(other),
                    },
                )?,
            );
        }
        match token {
            EpsToken::ProcedureStart => {
                if depth == limits.max_procedure_depth {
                    return Err(E::Limit);
                }
                stack[depth] = count;
                depth += 1;
            }
            EpsToken::ProcedureEnd => {
                depth = depth.checked_sub(1).ok_or(E::Procedure(count))?;
                instruction.procedure_partner = Some(stack[depth]);
            }
            _ => {}
        }
        emit(count, instruction);
        count += 1;
    }
    if depth != 0 {
        return Err(E::Procedure(stack[depth - 1]));
    }
    if cancelled() {
        return Err(E::Cancelled);
    }
    Ok(count)
}
/// Compile original EPS PostScript into a shared instruction tape. Numeric words
/// are cached, encoded strings are validated without decoding allocations,
/// and nested procedures receive brace indices. A validation/counting
/// pass uses no heap storage; exact tape bytes are admitted before allocation.
/// Names and encoded strings borrow source bytes for the program's lifetime;
/// source allocation accounting remains the caller's responsibility. This tape
/// is for lexical code segments: raw data consumed via `currentfile` must be
/// handled by a future stream-aware reader instead of precompiling a whole file.
/// It does not resolve immediate names, allocate PostScript string objects, execute
/// operators or render artwork. `[]` and `<<>>` remain executable tokens, not
/// scanner-level groups. Clone retains the managed tape until its last owner.
pub fn compile_eps_program<'a, F: FnMut() -> bool>(
    source: &EpsSource<'a>,
    limits: EpsCompileLimits,
    budget: &MemoryBudget,
    cancelled: F,
) -> Result<EpsProgram<'a>, EpsCompileError> {
    let callback = RefCell::new(cancelled);
    let check = || (callback.borrow_mut())();
    let length = walk(source, limits, &check, |_, _| {})?;
    let blank = EpsInstruction {
        token: EpsToken::Word(&[]),
        number: None,
        decoded_string_bytes: None,
        procedure_partner: None,
    };
    let mut instructions = budget.try_buffer(length, blank)?;
    walk(source, limits, &check, |index, instruction| {
        instructions[index] = instruction;
        if let Some(open) = instruction.procedure_partner {
            instructions[open].procedure_partner = Some(index);
        }
    })?;
    Ok(EpsProgram {
        instructions: instructions.freeze(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(bytes: &[u8]) -> EpsSource<'_> {
        EpsSource {
            postscript: bytes,
            wmf_preview: None,
            tiff_preview: None,
        }
    }
    #[test]
    fn nested_procedures_borrow_names_cache_numbers_and_preserve_runtime_marks() {
        let bytes =
            b"/average {add 2 div {16#FFFFFFFF} } def 40 60 average /123 //123 [ << ] >> <~z~> <F> (a\\n)";
        let budget = MemoryBudget::new(4096);
        let program =
            compile_eps_program(&source(bytes), EpsCompileLimits::default(), &budget, || false).unwrap();
        let tape = program.instructions();
        assert_eq!(tape.len(), 22);
        assert_eq!(tape[19].decoded_string_bytes, Some(4));
        assert_eq!(tape[20].decoded_string_bytes, Some(1));
        assert_eq!(tape[21].decoded_string_bytes, Some(2));
        assert_eq!(tape[1].procedure_partner, Some(8));
        assert_eq!(tape[8].procedure_partner, Some(1));
        assert_eq!(tape[5].procedure_partner, Some(7));
        assert_eq!(tape[7].procedure_partner, Some(5));
        assert_eq!(tape[3].number, Some(EpsNumber::Integer(2)));
        assert_eq!(tape[6].number, Some(EpsNumber::Integer(-1)));
        assert_eq!(tape[13].number, None);
        assert_eq!(tape[14].number, None);
        let EpsToken::LiteralName(name) = tape[0].token else {
            panic!()
        };
        assert_eq!(name.as_ptr(), bytes[1..].as_ptr());
        assert_eq!(
            budget.used(),
            (tape.len() * std::mem::size_of::<EpsInstruction<'_>>()) as u64
        );
        let held = program.clone();
        drop(program);
        assert!(budget.used() > 0);
        drop(held);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn malformed_procedures_and_limits_refuse_before_tape_allocation() {
        let limits = EpsCompileLimits {
            max_instructions: 16,
            max_token_bytes: 16,
            max_decoded_string_bytes: 16,
            max_procedure_depth: 2,
        };
        for bytes in [
            b"}".as_slice(),
            b"{ 1",
            b"{ { { } } }",
            b"12345678901234567",
            b"1e100",
            b"<~!z~>",
            b"<~uuuuu~>",
        ] {
            let budget = MemoryBudget::new(4096);
            assert!(compile_eps_program(&source(bytes), limits, &budget, || false).is_err());
            assert_eq!(budget.used(), 0);
            assert_eq!(budget.peak(), 0);
        }
        let budget = MemoryBudget::new(4096);
        let no_tokens = EpsCompileLimits {
            max_instructions: 0,
            ..limits
        };
        assert!(matches!(
            compile_eps_program(&source(b"1"), no_tokens, &budget, || false),
            Err(EpsCompileError::Lexical(EpsTokenError::Limit))
        ));
        let no_string = EpsCompileLimits {
            max_decoded_string_bytes: 0,
            ..limits
        };
        assert!(matches!(
            compile_eps_program(&source(b"<~z~>"), no_string, &budget, || false),
            Err(EpsCompileError::String(EpsStringError::Limit))
        ));
        let no_depth = EpsCompileLimits {
            max_procedure_depth: 0,
            ..limits
        };
        assert!(matches!(
            compile_eps_program(&source(b"{}"), no_depth, &budget, || false),
            Err(EpsCompileError::Limit)
        ));
        assert!(
            compile_eps_program(
                &source(b"% only comment"),
                no_tokens,
                &MemoryBudget::new(0),
                || false
            )
            .unwrap()
            .instructions()
            .is_empty()
        );
    }
    #[test]
    fn root_pressure_and_cancel_during_second_pass_release_tape() {
        let bytes = b"{ 1 2 add }";
        let src = source(bytes);
        let limits = EpsCompileLimits::default();
        let size = (5 * std::mem::size_of::<EpsInstruction<'_>>()) as u64;
        let root = MemoryBudget::new(size + 1);
        let input = root.try_buffer(2, 0u8).unwrap();
        assert!(matches!(
            compile_eps_program(&src, limits, &root.child(size), || false),
            Err(EpsCompileError::Memory(_))
        ));
        assert_eq!(root.used(), 2);
        drop(input);
        let mut polls = 0;
        // Discover the validation pass's poll count without guessing lexer details.
        let count = std::cell::Cell::new(0);
        walk(
            &src,
            limits,
            &|| {
                count.set(count.get() + 1);
                false
            },
            |_, _| {},
        )
        .unwrap();
        let cancel_at = count.get() + 4;
        assert!(matches!(
            compile_eps_program(&src, limits, &root, || {
                polls += 1;
                polls == cancel_at
            }),
            Err(EpsCompileError::Cancelled)
        ));
        assert_eq!(root.used(), 0);
        assert_eq!(root.peak(), size);
        assert_eq!(polls, cancel_at);
        let valid = compile_eps_program(&src, limits, &root, || false).unwrap();
        assert_eq!(root.used(), size);
        drop(valid);
        assert_eq!(root.used(), 0);
    }
}
