//! Atomic recovery command log, independent of XMP and session history schemas.
use crate::edit_store::SourceVersion;
use anyhow::{Result, bail};
use rrrah_core::edit::EditState;
use serde::{Deserialize, Serialize};
const SCHEMA: u32 = 2;
pub const MAX_COMMANDS: usize = 64;
type Digest = [u8; 32];
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Patch {
    exposure: Option<f32>,
    curve: Option<Vec<[f64; 2]>>,
}
impl Patch {
    fn apply(&self, mut state: EditState) -> Result<EditState> {
        if self.exposure.is_none() && self.curve.is_none() {
            bail!("empty recovery command");
        }
        if let Some(exposure) = self.exposure {
            state.exposure_stops = exposure;
        }
        if let Some(curve) = &self.curve {
            state = state.with_curve(curve)?;
        }
        state.validate()?;
        Ok(state)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    sequence: u64,
    previous: Digest,
    patch: Patch,
    checksum: Digest,
}
impl Command {
    fn digest(&self) -> Result<Digest> {
        Ok(*blake3::hash(&serde_json::to_vec(&(self.sequence, self.previous, &self.patch))?).as_bytes())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    schema: u32,
    source: SourceVersion,
    page: usize,
    checkpoint: EditState,
    sequence: u64,
    chain: Digest,
    commands: Vec<Command>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: Journal,
    checksum: Digest,
}
impl Journal {
    pub fn new(source: SourceVersion, page: usize, checkpoint: EditState) -> Result<Self> {
        checkpoint.validate()?;
        Ok(Self {
            schema: SCHEMA,
            source,
            page,
            checkpoint,
            sequence: 0,
            chain: [0; 32],
            commands: Vec::new(),
        })
    }
    pub fn replay(&self) -> Result<EditState> {
        if self.schema != SCHEMA || self.commands.len() > MAX_COMMANDS {
            bail!("unsupported or oversized recovery command log");
        }
        self.checkpoint.validate()?;
        let mut state = self.checkpoint;
        let mut sequence = self.sequence;
        let mut chain = self.chain;
        for command in &self.commands {
            sequence = sequence
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("recovery sequence overflow"))?;
            if command.sequence != sequence
                || command.previous != chain
                || command.digest()? != command.checksum
            {
                bail!("recovery command sequence/checksum mismatch");
            }
            state = command.patch.apply(state)?;
            chain = command.checksum;
        }
        Ok(state)
    }
    pub fn append(&mut self, target: EditState) -> Result<()> {
        target.validate()?;
        let current = self.replay()?;
        if current == target {
            return Ok(());
        }
        // Fold an old prefix, preserving every command after the new checkpoint.
        // Recovery is bounded; it does not promise unbounded persistent undo.
        if self.commands.len() == MAX_COMMANDS {
            let count = MAX_COMMANDS / 2;
            for command in &self.commands[..count] {
                self.checkpoint = command.patch.apply(self.checkpoint)?;
                self.sequence = command.sequence;
                self.chain = command.checksum;
            }
            self.commands.drain(..count);
        }
        let previous = self.commands.last().map_or(self.chain, |c| c.checksum);
        let sequence = self
            .commands
            .last()
            .map_or(self.sequence, |c| c.sequence)
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("recovery sequence overflow"))?;
        let mut command = Command {
            sequence,
            previous,
            patch: Patch {
                exposure: (current.exposure_stops != target.exposure_stops).then_some(target.exposure_stops),
                curve: (current.points() != target.points()).then(|| target.points().to_vec()),
            },
            checksum: [0; 32],
        };
        command.checksum = command.digest()?;
        self.commands.push(command);
        Ok(())
    }
    pub fn encode(self) -> Result<Vec<u8>> {
        self.replay()?;
        let checksum = *blake3::hash(&serde_json::to_vec(&self)?).as_bytes();
        Ok(serde_json::to_vec(&Envelope {
            payload: self,
            checksum,
        })?)
    }
}
/// None means the legacy snapshot reader must handle the document. Unknown
/// schemas fail closed in that reader, and are never migrated implicitly.
pub fn decode(bytes: &[u8], source: &SourceVersion, page: usize) -> Result<Option<Journal>> {
    #[derive(Deserialize)]
    struct Header {
        payload: Version,
    }
    #[derive(Deserialize)]
    struct Version {
        schema: u32,
    }
    if serde_json::from_slice::<Header>(bytes)?.payload.schema != SCHEMA {
        return Ok(None);
    }
    let envelope: Envelope = serde_json::from_slice(bytes)?;
    let journal = envelope.payload;
    if envelope.checksum != *blake3::hash(&serde_json::to_vec(&journal)?).as_bytes()
        || &journal.source != source
        || journal.page != page
    {
        bail!("recovery identity/checksum mismatch");
    }
    journal.replay()?;
    Ok(Some(journal))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> SourceVersion {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("source");
        std::fs::write(&p, b"image").unwrap();
        crate::edit_store::source_version(&p).unwrap()
    }
    #[test]
    fn replay_exposure_curve_undo_redo_and_bounded_compaction() {
        let source = source();
        let mut log = Journal::new(source.clone(), 3, EditState::default()).unwrap();
        for i in 0..1000 {
            let mut state = EditState::default()
                .with_curve(&[[0., 0.], [0.5, if i % 2 == 0 { 0.25 } else { 0.75 }], [1., 1.]])
                .unwrap();
            state.exposure_stops = (i % 20) as f32 - 10.;
            log.append(state).unwrap();
            assert_eq!(log.replay().unwrap(), state);
            assert!(log.commands.len() <= MAX_COMMANDS);
        }
        let state = log.replay().unwrap();
        let bytes = log.encode().unwrap();
        assert!(bytes.len() < 256 * 1024);
        assert_eq!(
            decode(&bytes, &source, 3).unwrap().unwrap().replay().unwrap(),
            state
        );
        assert!(decode(&bytes, &source, 2).is_err());
    }
    #[test]
    fn rejects_reordered_missing_corrupt_and_invalid_commands_even_with_valid_outer_checksum() {
        let source = source();
        let mut log = Journal::new(source.clone(), 0, EditState::default()).unwrap();
        let mut s = EditState::default();
        s.exposure_stops = 1.;
        log.append(s).unwrap();
        s.exposure_stops = 2.;
        log.append(s).unwrap();
        let bytes = log.encode().unwrap();
        for mutation in 0..4 {
            let mut e: Envelope = serde_json::from_slice(&bytes).unwrap();
            match mutation {
                0 => e.payload.commands.swap(0, 1),
                1 => {
                    e.payload.commands.remove(0);
                }
                2 => e.payload.commands[0].patch.exposure = Some(3.),
                _ => {
                    e.payload.commands[0].patch.exposure = Some(40.);
                    e.payload.commands[0].checksum = e.payload.commands[0].digest().unwrap();
                }
            }
            e.checksum = *blake3::hash(&serde_json::to_vec(&e.payload).unwrap()).as_bytes();
            assert!(decode(&serde_json::to_vec(&e).unwrap(), &source, 0).is_err());
        }
        for length in [0, 1, bytes.len() / 2, bytes.len() - 1] {
            assert!(decode(&bytes[..length], &source, 0).is_err());
        }
    }
}
