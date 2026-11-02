//! Experimental immutable chunk storage for transactional partitions.
//! Default builds keep flat storage; qualification adapters below are test-only.
//! Credits cover payload and explicit metadata allowances, not physical process RSS.
use super::{AdmittedVec, Admitter, Credit, Fragment, FragmentSink, PixelPartitionError, Result, tick};
#[cfg(test)]
use super::{LineKey, LinePool, OrderedPixel};
use std::{mem::size_of, ops::Range, sync::Arc};
const CHUNK_RECORDS: usize = 16;
#[derive(Clone, Copy)]
struct ChunkSummary {
    bounds: [f64; 4],
    axis: [f64; 2],
    projection: [f64; 2],
    known_edges: bool,
    eligible: bool,
}
impl ChunkSummary {
    fn empty() -> Self {
        Self {
            bounds: [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY],
            axis: [0.; 2],
            projection: [f64::INFINITY, f64::NEG_INFINITY],
            known_edges: true,
            eligible: true,
        }
    }
    // Outward-rounded intervals enclose the exact dot product of these
    // represented doubles, including cancellation and gradual underflow.
    fn project(axis: [f64; 2], point: kurbo::Point) -> Option<[f64; 2]> {
        let x = axis[0] * point.x;
        let y = axis[1] * point.y;
        let low = (x.next_down() + y.next_down()).next_down();
        let high = (x.next_up() + y.next_up()).next_up();
        (low.is_finite() && high.is_finite()).then_some([low, high])
    }
    fn include(&mut self, f: &Fragment) {
        self.eligible &= f.has_area && f.no_merge_with_previous;
        self.known_edges &= f.polygon.incoming[..f.polygon.len].iter().all(|id| *id != 0);
        if self.bounds[0] == f64::INFINITY {
            let mut longest = 0.;
            for [a, b] in f.polygon.edges() {
                let d = b - a;
                let length = d.x.abs().max(d.y.abs());
                if length.is_finite() && length > longest {
                    self.axis = [-d.y, d.x];
                    longest = length;
                }
            }
        }
        for p in &f.polygon.points[..f.polygon.len] {
            self.bounds[0] = self.bounds[0].min(p.x);
            self.bounds[1] = self.bounds[1].min(p.y);
            self.bounds[2] = self.bounds[2].max(p.x);
            self.bounds[3] = self.bounds[3].max(p.y);
            if self.axis != [0.; 2] {
                if let Some([low, high]) = Self::project(self.axis, *p) {
                    self.projection[0] = self.projection[0].min(low);
                    self.projection[1] = self.projection[1].max(high);
                } else {
                    self.projection = [f64::NEG_INFINITY, f64::INFINITY];
                }
            }
        }
    }
    fn from_values(values: &[Fragment]) -> Self {
        let mut summary = Self::empty();
        for f in values {
            summary.include(f);
        }
        summary
    }
    fn disjoint(&self, cut: &super::Polygon, work: &mut usize, cancelled: &dyn Fn() -> bool) -> Result<bool> {
        let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for p in &cut.points[..cut.len] {
            b[0] = b[0].min(p.x);
            b[1] = b[1].min(p.y);
            b[2] = b[2].max(p.x);
            b[3] = b[3].max(p.y);
        }
        if !(self.bounds[0] < b[2] && b[0] < self.bounds[2] && self.bounds[1] < b[3] && b[1] < self.bounds[3])
        {
            return Ok(true);
        }
        // An unknown support can be interned by the ordinary overlay even
        // when its final intersection is empty. Keep that path so future
        // canonical line identities remain identical to the flat backend.
        if !self.known_edges
            || cut.incoming[..cut.len].contains(&0)
            || self.axis == [0.; 2]
            || !self.projection.iter().all(|v| v.is_finite())
        {
            return Ok(false);
        }
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for point in &cut.points[..cut.len] {
            tick(work, cancelled)?;
            let Some([a, b]) = Self::project(self.axis, *point) else {
                return Ok(false);
            };
            low = low.min(a);
            high = high.max(b);
        }
        Ok(self.projection[1] < low || high < self.projection[0])
    }
}
struct Chunk {
    values: Vec<Fragment>,
    summary: ChunkSummary,
    _credit: Credit,
}
#[derive(Clone)]
struct Segment {
    chunk: Arc<Chunk>,
    start: usize,
    len: usize,
    end: usize,
}
pub(super) struct SharedFragments<'a> {
    segments: AdmittedVec<'a, Segment>,
    len: usize,
    maximum: usize,
    last_ineligible_end: usize,
}
fn allocate_chunk(input: &[Fragment], capacity: usize, admit: &Admitter<'_>) -> Result<Arc<Chunk>> {
    if input.is_empty() || capacity < input.len() || capacity > CHUNK_RECORDS {
        return Err(PixelPartitionError::Invalid);
    }
    let bytes = capacity
        .checked_mul(size_of::<Fragment>())
        .and_then(|n| n.checked_add(size_of::<Chunk>() + 4 * size_of::<usize>()))
        .ok_or(PixelPartitionError::Limit)?;
    let credit = admit(bytes).ok_or(PixelPartitionError::Admission)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| PixelPartitionError::Admission)?;
    if values.capacity() > capacity {
        return Err(PixelPartitionError::Admission);
    }
    values.extend_from_slice(input);
    let summary = ChunkSummary::from_values(&values);
    Ok(Arc::new(Chunk {
        values,
        summary,
        _credit: credit,
    }))
}
impl<'a> SharedFragments<'a> {
    fn empty(admit: &'a Admitter<'a>, maximum: usize) -> Self {
        Self {
            segments: AdmittedVec::new(admit, maximum),
            len: 0,
            maximum,
            last_ineligible_end: 0,
        }
    }
    fn check_len(&self, extra: usize) -> Result<usize> {
        self.len
            .checked_add(extra)
            .filter(|n| *n <= self.maximum)
            .ok_or(PixelPartitionError::Limit)
    }
    fn append_chunk(&mut self, input: &[Fragment], capacity: usize) -> Result<()> {
        if input.is_empty() || capacity < input.len() || capacity > CHUNK_RECORDS {
            return Err(PixelPartitionError::Invalid);
        }
        let end = self.check_len(input.len())?;
        let chunk = allocate_chunk(input, capacity, self.segments.admit)?;
        let eligible = chunk.summary.eligible;
        self.segments.push(Segment {
            chunk,
            start: 0,
            len: input.len(),
            end,
        })?;
        if !eligible { self.last_ineligible_end = end; }
        self.len = end;
        Ok(())
    }
    fn append_values(
        &mut self,
        values: &[Fragment],
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        self.check_len(values.len())?;
        for input in values.chunks(CHUNK_RECORDS) {
            tick(work, cancelled)?;
            self.append_chunk(input, input.len())?;
        }
        Ok(())
    }
    fn push_fragment(&mut self, item: Fragment) -> Result<()> {
        self.check_len(1)?;
        let remaining = self.maximum - self.len;
        let admit = self.segments.admit;
        if let Some(last) = self.segments.values.last_mut() {
            if last.start == 0 && last.len == last.chunk.values.len() {
                if let Some(chunk) = Arc::get_mut(&mut last.chunk) {
                    if chunk.values.len() == chunk.values.capacity()
                        && chunk.values.capacity() < CHUNK_RECORDS
                    {
                        let capacity = (chunk.values.capacity() * 2)
                            .min(CHUNK_RECORDS)
                            .min(chunk.values.len() + remaining);
                        let bytes = capacity
                            .checked_mul(size_of::<Fragment>())
                            .and_then(|n| n.checked_add(size_of::<Chunk>() + 4 * size_of::<usize>()))
                            .ok_or(PixelPartitionError::Limit)?;
                        let credit = admit(bytes).ok_or(PixelPartitionError::Admission)?;
                        chunk
                            .values
                            .try_reserve_exact(capacity - chunk.values.len())
                            .map_err(|_| PixelPartitionError::Admission)?;
                        if chunk.values.capacity() > capacity {
                            return Err(PixelPartitionError::Admission);
                        }
                        chunk._credit = credit;
                    }
                    if chunk.values.len() < chunk.values.capacity() {
                        chunk.values.push(item);
                        chunk.summary.include(&item);
                        last.len += 1;
                        last.end += 1;
                        self.len += 1;
                        if !chunk.summary.eligible { self.last_ineligible_end = self.len; }
                        return Ok(());
                    }
                }
            }
        }
        if let Some(last) = self.segments.values.last_mut() {
            if last.len < CHUNK_RECORDS {
                let mut scratch = [item; CHUNK_RECORDS];
                scratch[..last.len].copy_from_slice(&last.chunk.values[last.start..last.start + last.len]);
                scratch[last.len] = item;
                let capacity = (last.len + 1)
                    .next_power_of_two()
                    .max(4)
                    .min(CHUNK_RECORDS)
                    .min(last.len + remaining);
                let chunk = allocate_chunk(&scratch[..last.len + 1], capacity, admit)?;
                last.chunk = chunk;
                last.start = 0;
                last.len += 1;
                last.end += 1;
                self.len += 1;
                if !last.chunk.summary.eligible { self.last_ineligible_end = self.len; }
                return Ok(());
            }
        }
        self.append_chunk(&[item], 4.min(remaining))
    }
    fn truncate(&mut self, target: usize) {
        while self.len > target {
            let last = self.segments.values.last_mut().unwrap();
            let first = last.end - last.len;
            if target <= first {
                self.segments.values.pop();
                self.len = first;
            } else {
                last.len = target - first;
                last.end = target;
                self.len = target;
                if last.start == 0 {
                    if let Some(chunk) = Arc::get_mut(&mut last.chunk) {
                        chunk.values.truncate(last.len);
                        chunk.summary = ChunkSummary::from_values(&chunk.values);
                    }
                }
            }
        }
        // Removing a bad tail can leave a conservative boundary at the new
        // end. Appended eligible chunks can still form a fresh carry suffix.
        self.last_ineligible_end = self.last_ineligible_end.min(self.len);
    }
    fn append_range(
        &mut self,
        source: &Self,
        range: Range<usize>,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        if range.start > range.end || range.end > source.len {
            return Err(PixelPartitionError::Invalid);
        }
        self.check_len(range.end - range.start)?;
        let mut cursor = range.start;
        let mut index = source.segments.values.partition_point(|s| s.end <= cursor);
        while cursor < range.end {
            tick(work, cancelled)?;
            let old = &source.segments.values[index];
            let old_first = old.end - old.len;
            let first = cursor - old_first;
            let count = (old.len - first).min(range.end - cursor);
            let end = self.check_len(count)?;
            self.segments.push(Segment {
                chunk: Arc::clone(&old.chunk),
                start: old.start + first,
                len: count,
                end,
            })?;
            if !old.chunk.summary.eligible { self.last_ineligible_end = end; }
            self.len = end;
            cursor += count;
            index += 1;
        }
        Ok(())
    }
    fn from_values(
        values: &[Fragment],
        admit: &'a Admitter<'a>,
        maximum: usize,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self> {
        let mut next = Self::empty(admit, maximum);
        tick(work, cancelled)?;
        next.append_values(values, work, cancelled)?;
        Ok(next)
    }
    fn with_tail(
        &self,
        prefix: usize,
        tail: &[Fragment],
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self> {
        if prefix > self.len {
            return Err(PixelPartitionError::Invalid);
        }
        prefix
            .checked_add(tail.len())
            .filter(|n| *n <= self.maximum)
            .ok_or(PixelPartitionError::Limit)?;
        tick(work, cancelled)?;
        let mut next = Self::empty(self.segments.admit, self.maximum);
        next.append_range(self, 0..prefix, work, cancelled)?;
        next.append_values(tail, work, cancelled)?;
        tick(work, cancelled)?;
        Ok(next)
    }
    fn range(&self, range: Range<usize>, work: &mut usize, cancelled: &dyn Fn() -> bool) -> Result<Self> {
        tick(work, cancelled)?;
        let mut next = Self::empty(self.segments.admit, self.maximum);
        next.append_range(self, range, work, cancelled)?;
        tick(work, cancelled)?;
        Ok(next)
    }
    fn get(&self, index: usize) -> Option<&Fragment> {
        if index >= self.len {
            return None;
        }
        let segment = &self.segments.values[self.segments.values.partition_point(|s| s.end <= index)];
        segment
            .chunk
            .values
            .get(segment.start + index - (segment.end - segment.len))
    }
    fn descriptor_bytes(&self) -> usize {
        self.segments.values.capacity() * size_of::<Segment>()
    }
}

#[cfg(test)]
std::thread_local! {static USE_SHARED:std::cell::Cell<bool>=const {std::cell::Cell::new(false)};}
#[cfg(test)]
pub(super) fn set_shared(enabled: bool) -> bool {
    USE_SHARED.with(|v| v.replace(enabled))
}
pub(super) enum RegionValues<'a> {
    Flat(AdmittedVec<'a, Fragment>),
    Shared(SharedFragments<'a>),
}
impl<'a> RegionValues<'a> {
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Flat(v) => v.values.len(),
            Self::Shared(v) => v.len,
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(super) fn get(&self, i: usize) -> Option<&Fragment> {
        match self {
            Self::Flat(v) => v.values.get(i),
            Self::Shared(v) => v.get(i),
        }
    }
    pub(super) fn last(&self) -> Option<&Fragment> {
        self.get(self.len().checked_sub(1)?)
    }
    pub(super) fn iter(&self) -> RegionIter<'_, 'a> {
        RegionIter {
            values: self,
            index: 0,
        }
    }
}
pub(super) struct RegionIter<'s, 'a> {
    values: &'s RegionValues<'a>,
    index: usize,
}
impl<'s, 'a> Iterator for RegionIter<'s, 'a> {
    type Item = &'s Fragment;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.values.len() {
            return None;
        }
        let i = self.index;
        self.index += 1;
        self.values.get(i)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.values.len() - self.index;
        (n, Some(n))
    }
}
impl ExactSizeIterator for RegionIter<'_, '_> {}
impl<'s, 'a> IntoIterator for &'s RegionValues<'a> {
    type Item = &'s Fragment;
    type IntoIter = RegionIter<'s, 'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl std::ops::Index<usize> for RegionValues<'_> {
    type Output = Fragment;
    fn index(&self, i: usize) -> &Fragment {
        self.get(i).expect("region index out of bounds")
    }
}
#[cfg(test)]
impl std::ops::Deref for RegionValues<'_> {
    type Target = [Fragment];
    fn deref(&self) -> &[Fragment] {
        match self {
            Self::Flat(v) => &v.values,
            Self::Shared(_) => {
                panic!("flat-only qualification adapter cannot inspect shared storage as a slice")
            }
        }
    }
}
#[cfg(test)]
macro_rules! flat_range_index {
    ($($range:ty),*) => {$ (
        impl std::ops::Index<$range> for RegionValues<'_> {
            type Output=[Fragment];
            fn index(&self,index:$range)->&[Fragment] {&std::ops::Deref::deref(self)[index]}
        }
    )*};
}
#[cfg(test)]
flat_range_index!(
    std::ops::Range<usize>,
    std::ops::RangeFrom<usize>,
    std::ops::RangeTo<usize>,
    std::ops::RangeFull,
    std::ops::RangeInclusive<usize>,
    std::ops::RangeToInclusive<usize>
);
pub(super) struct RegionOutput<'a> {
    pub(super) values: RegionValues<'a>,
    pub(super) maximum: usize,
    pub(super) merge_floor: usize,
    pub(super) admit: &'a Admitter<'a>,
}
impl<'a> RegionOutput<'a> {
    pub(super) fn new(admit: &'a Admitter<'a>, maximum: usize) -> Self {
        #[cfg(not(test))]
        let shared = cfg!(feature = "experimental-shared-partitions");
        #[cfg(test)]
        let shared = USE_SHARED.with(|v| v.get());
        Self {
            values: if shared {
                RegionValues::Shared(SharedFragments::empty(admit, maximum))
            } else {
                RegionValues::Flat(AdmittedVec::new(admit, maximum))
            },
            maximum,
            merge_floor: 0,
            admit,
        }
    }
    pub(super) fn empty_like(&self) -> Self {
        Self {
            values: match &self.values {
                RegionValues::Flat(_) => RegionValues::Flat(AdmittedVec::new(self.admit, self.maximum)),
                RegionValues::Shared(_) => {
                    RegionValues::Shared(SharedFragments::empty(self.admit, self.maximum))
                }
            },
            maximum: self.maximum,
            merge_floor: 0,
            admit: self.admit,
        }
    }
    pub(super) fn carry_span(
        &self,
        start: usize,
        cut: &super::Polygon,
        empty: bool,
        room: usize,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<(usize, bool)>> {
        let RegionValues::Shared(values) = &self.values else {
            return Ok(None);
        };
        // The matching old prefix is already retained. With no incoming
        // remainder and a proven nonmerging positive suffix, carry it at once.
        // Require enough output room so a failed paint never pre-scans beyond
        // the old single-segment overflow boundary.
        if empty && start >= values.last_ineligible_end && start < values.len && values.len - start <= room {
            return Ok(Some((values.len, true)));
        }
        let index = values.segments.values.partition_point(|s| s.end <= start);
        let Some(segment) = values.segments.values.get(index) else { return Ok(None); };
        Ok(Some((
            segment.end,
            segment.chunk.summary.eligible && (empty || segment.chunk.summary.disjoint(cut, work, cancelled)?),
        )))
    }
    pub(super) fn copy_from(
        &mut self,
        source: &Self,
        start: usize,
        end: usize,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        match (&mut self.values, &source.values) {
            (RegionValues::Flat(out), RegionValues::Flat(old)) => {
                out.extend_copy(&old.values[start..end], work, cancelled)
            }
            (RegionValues::Shared(out), RegionValues::Shared(old)) => {
                out.append_range(old, start..end, work, cancelled)
            }
            _ => Err(PixelPartitionError::Invalid),
        }
    }
    pub(super) fn restore_prefix(&mut self, prefix: usize, previous: Option<Fragment>) -> Result<()> {
        match &mut self.values {
            RegionValues::Flat(v) => {
                v.values.truncate(prefix);
                if let Some(previous) = previous {
                    v.values[prefix - 1] = previous;
                }
            }
            RegionValues::Shared(v) => {
                v.truncate(prefix);
                if let Some(previous) = previous {
                    v.truncate(prefix - 1);
                    v.push_fragment(previous)?;
                }
            }
        }
        Ok(())
    }
}
impl FragmentSink for RegionOutput<'_> {
    fn sink_len(&self) -> usize {
        self.values.len()
    }
    fn sink_last(&self) -> Option<&Fragment> {
        self.values.last()
    }
    fn sink_pop(&mut self) {
        match &mut self.values {
            RegionValues::Flat(v) => {
                v.values.pop();
            }
            RegionValues::Shared(v) => {
                v.truncate(v.len - 1);
            }
        }
    }
    fn sink_push(&mut self, item: Fragment) -> Result<()> {
        match &mut self.values {
            RegionValues::Flat(v) => v.push(item),
            RegionValues::Shared(v) => v.push_fragment(item),
        }
    }
    fn sink_floor(&self) -> usize {
        self.merge_floor
    }
}

#[cfg(test)]
fn same_fragment(a: &Fragment, b: &Fragment) -> bool {
    a.owner == b.owner
        && a.owner.u.map(f64::to_bits) == b.owner.u.map(f64::to_bits)
        && a.owner.v.map(f64::to_bits) == b.owner.v.map(f64::to_bits)
        && a.owner.components.map(|f| f.map(f64::to_bits)) == b.owner.components.map(|f| f.map(f64::to_bits))
        && a.polygon.len == b.polygon.len
        && a.polygon.points[..a.polygon.len]
            .iter()
            .zip(&b.polygon.points[..b.polygon.len])
            .all(|(a, b)| [a.x.to_bits(), a.y.to_bits()] == [b.x.to_bits(), b.y.to_bits()])
        && a.polygon.incoming[..a.polygon.len] == b.polygon.incoming[..b.polygon.len]
        && a.has_area == b.has_area
        && a.no_merge_with_previous == b.no_merge_with_previous
}

#[cfg(test)]
pub(super) fn next_pool_id() -> usize {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("test line-pool identity space exhausted")
}
#[cfg(test)]
struct LineContext {
    identity: usize,
    keys: Vec<LineKey>,
    _credit: Credit,
}
#[cfg(test)]
impl LineContext {
    fn capture(
        pool: &LinePool<'_>,
        admit: &Admitter<'_>,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Arc<Self>> {
        tick(work, cancelled)?;
        if pool.keys.len().checked_add(1) != Some(pool.next as usize) {
            return Err(PixelPartitionError::Invalid);
        }
        let bytes = pool
            .keys
            .len()
            .checked_mul(size_of::<LineKey>())
            .and_then(|n| n.checked_add(size_of::<Self>() + 4 * size_of::<usize>()))
            .ok_or(PixelPartitionError::Limit)?;
        let credit = admit(bytes).ok_or(PixelPartitionError::Admission)?;
        let mut keys = Vec::new();
        keys.try_reserve_exact(pool.keys.len())
            .map_err(|_| PixelPartitionError::Admission)?;
        if keys.capacity() > pool.keys.len() {
            return Err(PixelPartitionError::Admission);
        }
        for &key in &pool.keys {
            tick(work, cancelled)?;
            keys.push(key);
        }
        Ok(Arc::new(Self {
            identity: pool.identity,
            keys,
            _credit: credit,
        }))
    }
    fn resolves(&self, id: u32) -> Option<LineKey> {
        self.keys.get(id.checked_sub(1)? as usize).copied()
    }
    fn accepts_pool(
        &self,
        pool: &LinePool<'_>,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        tick(work, cancelled)?;
        if self.identity != pool.identity || self.keys.len() > pool.keys.len() {
            return Err(PixelPartitionError::Invalid);
        }
        for (a, b) in self.keys.iter().zip(&pool.keys) {
            tick(work, cancelled)?;
            if a != b {
                return Err(PixelPartitionError::Invalid);
            }
        }
        Ok(())
    }
    fn validate_tags(
        &self,
        fragments: &[Fragment],
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        for f in fragments {
            tick(work, cancelled)?;
            if f.polygon.incoming[..f.polygon.len]
                .iter()
                .any(|id| *id != 0 && self.resolves(*id).is_none())
            {
                return Err(PixelPartitionError::Invalid);
            }
        }
        Ok(())
    }
}
#[cfg(test)]
enum SnapshotPiece<'s, 'a> {
    Shared {
        source: &'s BoundSnapshot<'a>,
        range: Range<usize>,
    },
    New(&'s [Fragment]),
}
#[cfg(test)]
struct BoundSnapshot<'a> {
    payload: SharedFragments<'a>,
    context: Arc<LineContext>,
}
#[cfg(test)]
impl<'a> BoundSnapshot<'a> {
    fn capture(
        pixel: &OrderedPixel<'_>,
        admit: &'a Admitter<'a>,
        maximum: usize,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self> {
        let context = LineContext::capture(&pixel.lines, admit, work, cancelled)?;
        context.validate_tags(&pixel.regions.values, work, cancelled)?;
        let payload = SharedFragments::from_values(&pixel.regions.values, admit, maximum, work, cancelled)?;
        tick(work, cancelled)?;
        Ok(Self { payload, context })
    }
    fn compacted(&self, work: &mut usize, cancelled: &dyn Fn() -> bool) -> Result<Self> {
        tick(work, cancelled)?;
        let mut payload = SharedFragments::empty(self.payload.segments.admit, self.payload.maximum);
        if self.payload.len > 0 {
            let mut scratch = [*self.payload.get(0).ok_or(PixelPartitionError::Invalid)?; CHUNK_RECORDS];
            for start in (0..self.payload.len).step_by(CHUNK_RECORDS) {
                let count = (self.payload.len - start).min(CHUNK_RECORDS);
                for (i, value) in scratch[..count].iter_mut().enumerate() {
                    tick(work, cancelled)?;
                    *value = *self.payload.get(start + i).ok_or(PixelPartitionError::Invalid)?;
                }
                payload.append_values(&scratch[..count], work, cancelled)?;
            }
        }
        tick(work, cancelled)?;
        Ok(Self {
            payload,
            context: Arc::clone(&self.context),
        })
    }
    fn checked_partition_pieces(
        pixel: &OrderedPixel<'_>,
        pieces: &[SnapshotPiece<'_, 'a>],
        admit: &'a Admitter<'a>,
        maximum: usize,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self> {
        tick(work, cancelled)?;
        let mut count = 0usize;
        for piece in pieces {
            tick(work, cancelled)?;
            let len = match piece {
                SnapshotPiece::Shared { source, range } => {
                    if range.start > range.end || range.end > source.payload.len {
                        return Err(PixelPartitionError::Invalid);
                    }
                    source.context.accepts_pool(&pixel.lines, work, cancelled)?;
                    range.end - range.start
                }
                SnapshotPiece::New(values) => values.len(),
            };
            count = count
                .checked_add(len)
                .filter(|n| *n <= maximum)
                .ok_or(PixelPartitionError::Limit)?;
        }
        if count != pixel.fragments() {
            return Err(PixelPartitionError::Invalid);
        }
        let context = LineContext::capture(&pixel.lines, admit, work, cancelled)?;
        let mut payload = SharedFragments::empty(admit, maximum);
        for piece in pieces {
            tick(work, cancelled)?;
            match piece {
                SnapshotPiece::Shared { source, range } => {
                    payload.append_range(&source.payload, range.clone(), work, cancelled)?
                }
                SnapshotPiece::New(values) => {
                    context.validate_tags(values, work, cancelled)?;
                    payload.append_values(values, work, cancelled)?;
                }
            }
        }
        for (i, actual) in pixel.regions.values.iter().enumerate() {
            tick(work, cancelled)?;
            if !same_fragment(payload.get(i).ok_or(PixelPartitionError::Invalid)?, actual) {
                return Err(PixelPartitionError::Invalid);
            }
        }
        tick(work, cancelled)?;
        Ok(Self { payload, context })
    }
    // This checked adapter scans the claimed prefix. The normal partition will
    // need carry proofs, not this validation scan, for a useful performance path.
    fn checked_partition_tail(
        &self,
        pixel: &OrderedPixel<'_>,
        prefix: usize,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self> {
        self.context.accepts_pool(&pixel.lines, work, cancelled)?;
        if prefix > self.payload.len || prefix > pixel.fragments() {
            return Err(PixelPartitionError::Invalid);
        }
        for (i, f) in pixel.regions.values[..prefix].iter().enumerate() {
            tick(work, cancelled)?;
            if !same_fragment(self.payload.get(i).ok_or(PixelPartitionError::Invalid)?, f) {
                return Err(PixelPartitionError::Invalid);
            }
        }
        let context = LineContext::capture(&pixel.lines, self.payload.segments.admit, work, cancelled)?;
        context.validate_tags(&pixel.regions.values[prefix..], work, cancelled)?;
        let payload = self
            .payload
            .with_tail(prefix, &pixel.regions.values[prefix..], work, cancelled)?;
        tick(work, cancelled)?;
        Ok(Self { payload, context })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        sync::atomic::{AtomicUsize, Ordering},
    };
    struct Charge {
        used: Arc<AtomicUsize>,
        bytes: usize,
    }
    impl Drop for Charge {
        fn drop(&mut self) {
            self.used.fetch_sub(self.bytes, Ordering::SeqCst);
        }
    }
    struct Ledger {
        used: Arc<AtomicUsize>,
        calls: Cell<usize>,
        reject: Cell<usize>,
        limit: Cell<usize>,
    }
    impl Ledger {
        fn new() -> Self {
            Self {
                used: Arc::new(AtomicUsize::new(0)),
                calls: Cell::new(0),
                reject: Cell::new(usize::MAX),
                limit: Cell::new(16 * 1024 * 1024),
            }
        }
        fn reserve(&self, bytes: usize) -> Option<Credit> {
            let at = self.calls.get();
            self.calls.set(at + 1);
            let used = self.used();
            if at == self.reject.get() || used.checked_add(bytes)? > self.limit.get() {
                return None;
            }
            self.used.fetch_add(bytes, Ordering::SeqCst);
            Some(Box::new(Charge {
                used: Arc::clone(&self.used),
                bytes,
            }))
        }
        fn used(&self) -> usize {
            self.used.load(Ordering::SeqCst)
        }
    }
    fn fragment(index: usize) -> Fragment {
        let mut polygon = super::super::Polygon::empty();
        let x = index as f64 / 512.;
        for p in [
            kurbo::Point::new(x, 0.),
            kurbo::Point::new(x + 1. / 1024., 0.),
            kurbo::Point::new(x, 1. / 1024.),
        ] {
            polygon.push(p).unwrap();
        }
        Fragment::new(polygon, super::super::PixelSourceOwner {
                patch_index: index,
                sample_index: index,
                component_count: 3,
                u: [x, 1., 0.],
                v: [0., 0., 1.],
                components: [[x, 1., 0.], [0.5, 0., 0.], [0.25, 0., 0.], [0.; 3]],
            },
            true, true,
        )
    }
    fn assert_fragment(a: &Fragment, b: &Fragment) {
        assert_eq!(a.owner, b.owner);
        assert_eq!(a.owner.u.map(f64::to_bits), b.owner.u.map(f64::to_bits));
        assert_eq!(a.owner.v.map(f64::to_bits), b.owner.v.map(f64::to_bits));
        assert_eq!(
            a.owner.components.map(|f| f.map(f64::to_bits)),
            b.owner.components.map(|f| f.map(f64::to_bits))
        );
        assert_eq!(a.polygon.len, b.polygon.len);
        for (a, b) in a.polygon.points[..a.polygon.len]
            .iter()
            .zip(&b.polygon.points[..b.polygon.len])
        {
            assert_eq!([a.x.to_bits(), a.y.to_bits()], [b.x.to_bits(), b.y.to_bits()]);
        }
        assert_eq!(
            a.polygon.incoming[..a.polygon.len],
            b.polygon.incoming[..b.polygon.len]
        );
        assert_eq!(a.has_area, b.has_area);
        assert_eq!(a.no_merge_with_previous, b.no_merge_with_previous);
        #[cfg(feature = "experimental-shared-partitions")]
        for f in [a,b] {
            let expected = if f.has_area { super::super::FragmentBounds::new(&f.polygon) }
                else { super::super::FragmentBounds::DISABLED };
            assert_eq!(f.bounds.axis,expected.axis);
            assert_eq!(f.bounds.interval,expected.interval);
        }
    }
    #[test]
    fn empty_incoming_carries_whole_eligible_suffix_without_overflow_prescan() {
        use std::cell::Cell;
        let ledger=Ledger::new();let calls=Cell::new(0usize);let fail=Cell::new(usize::MAX);
        let admit=|n| {calls.set(calls.get()+1);if calls.get()==fail.get(){None}else{ledger.reserve(n)}};
        let mut source=mode_pixel(&admit,256,true);
        for i in 0..128 {source.regions.sink_push(fragment(i)).unwrap();}
        assert_summaries(&source);
        let cut=super::super::Polygon::empty();
        assert_eq!(source.regions.carry_span(3,&cut,true,125,&mut 100,&||false).unwrap(),Some((128,true)));
        assert_eq!(source.regions.carry_span(3,&cut,true,124,&mut 100,&||false).unwrap(),Some((16,true)));
        assert_eq!(source.regions.carry_span(128,&cut,true,256,&mut 100,&||false).unwrap(),None);
        let copy=|work:&mut usize,cancelled:&dyn Fn()->bool| -> Result<RegionOutput<'_>> {
            let mut out=source.regions.empty_like();
            out.copy_from(&source.regions,0,3,work,cancelled)?;
            let (end,eligible)=source.regions.carry_span(3,&cut,true,125,work,cancelled)?.unwrap();
            assert!(eligible);tick(work,cancelled)?;
            out.copy_from(&source.regions,3,end,work,cancelled)?;Ok(out)
        };
        let checks=Cell::new(0usize);calls.set(0);let mut work=10000;
        let fast=copy(&mut work,&||{checks.set(checks.get()+1);false}).unwrap();
        let cost=10000-work;let allocations=calls.get();
        let mut slow=source.regions.empty_like();let mut slow_work=10000;
        slow.copy_from(&source.regions,0,3,&mut slow_work,&||false).unwrap();
        let mut cursor=3;
        while cursor<128 {
            let end=((cursor/16+1)*16).min(128);tick(&mut slow_work,&||false).unwrap();
            slow.copy_from(&source.regions,cursor,end,&mut slow_work,&||false).unwrap();cursor=end;
        }
        assert!(cost<10000-slow_work);
        for i in 0..128 {assert_fragment(&source.regions.values[i],&fast.values[i]);assert_fragment(&source.regions.values[i],&slow.values[i]);}
        drop(fast);drop(slow);let baseline=ledger.used();
        for at in 1..=allocations {calls.set(0);fail.set(at);assert!(matches!(copy(&mut 10000,&||false),Err(PixelPartitionError::Admission)));assert_eq!(ledger.used(),baseline);}
        fail.set(usize::MAX);
        for stop in 0..checks.get() {
            let at=Cell::new(0usize);
            assert!(matches!(copy(&mut 10000,&||{let now=at.get();at.set(now+1);now==stop}),Err(PixelPartitionError::Cancelled)));
            assert_eq!(ledger.used(),baseline);
        }
        for mut quota in 0..cost {assert!(matches!(copy(&mut quota,&||false),Err(PixelPartitionError::Limit)));assert_eq!(ledger.used(),baseline);}
        let retry=copy(&mut cost.clone(),&||false).unwrap();for i in 0..128 {assert_fragment(&source.regions.values[i],&retry.values[i]);}drop(retry);
        assert_eq!(ledger.used(),baseline);
        let RegionValues::Shared(payload)=&mut source.regions.values else {panic!()};
        let view=payload.range(0..64,&mut 1000,&||false).unwrap();
        payload.truncate(63);payload.push_fragment(fragment(999)).unwrap();assert_eq!(payload.last_ineligible_end,0);
        assert_eq!(view.get(63).unwrap().owner.patch_index,63);assert_eq!(payload.get(63).unwrap().owner.patch_index,999);
        let mut bad=fragment(1000);bad.no_merge_with_previous=false;payload.push_fragment(bad).unwrap();assert_ne!(payload.last_ineligible_end,0);
        payload.truncate(64);assert_ne!(payload.last_ineligible_end,0); // conservative after removing bad tail
        let recovered=payload.range(0..64,&mut 1000,&||false).unwrap();assert_eq!(recovered.last_ineligible_end,0);
        payload.truncate(0);assert_eq!(payload.last_ineligible_end,0);payload.push_fragment(fragment(7)).unwrap();assert_eq!(payload.last_ineligible_end,0);
        drop(view);drop(recovered);assert_summaries(&source);drop(source);assert_eq!(ledger.used(),0);
        eprintln!("EMPTY_CARRY suffix125 chunks8 old_work={} new_work={cost}; admissions={allocations} cancellations={} quotas={cost}; exact fields/tags, room fallback, COW/truncate/retry; final=0",10000-slow_work,checks.get());
    }
    #[test]
    fn carry_boundary_tracks_guarded_chunks_across_ranges_and_tail_growth() {
        let ledger=Ledger::new();let admit=|n|ledger.reserve(n);
        for bad in [0,20,63,127] {
            let mut input=(0..128).map(fragment).collect::<Vec<_>>();input[bad].no_merge_with_previous=false;
            let payload=SharedFragments::from_values(&input,&admit,256,&mut 1000,&||false).unwrap();
            let boundary=(bad/16+1)*16;assert_eq!(payload.last_ineligible_end,boundary);
            let mut pixel=mode_pixel(&admit,256,true);pixel.regions.values=RegionValues::Shared(payload);assert_summaries(&pixel);
            for start in 0..128 {
                let expected=if start>=boundary {(128,true)}else{((start/16+1)*16,start/16!=bad/16)};
                assert_eq!(pixel.regions.carry_span(start,&super::super::Polygon::empty(),true,256,&mut 1000,&||false).unwrap(),Some(expected));
            }
            if boundary<128 {
                assert_eq!(pixel.regions.carry_span(boundary,&super::super::Polygon::empty(),true,127-boundary,&mut 1000,&||false).unwrap(),Some((boundary+16,true)));
            }
            let RegionValues::Shared(values)=&pixel.regions.values else {panic!()};
            let view=values.range(1..128,&mut 1000,&||false).unwrap();assert_eq!(view.last_ineligible_end,boundary-1);
            let good=values.range(boundary..128,&mut 1000,&||false).unwrap();assert_eq!(good.last_ineligible_end,0);
            drop(view);drop(good);drop(pixel);assert_eq!(ledger.used(),0);
        }
        let mut bad=fragment(0);bad.no_merge_with_previous=false;
        let mut payload=SharedFragments::empty(&admit,256);payload.push_fragment(bad).unwrap();
        for i in 1..17 {payload.push_fragment(fragment(i)).unwrap();assert_eq!(payload.last_ineligible_end,(i+1).min(16));}
        let view=payload.range(0..17,&mut 1000,&||false).unwrap();payload.truncate(2);
        payload.push_fragment(fragment(18)).unwrap();assert_eq!(payload.last_ineligible_end,3); // COW still includes guard
        payload.truncate(0);payload.push_fragment(fragment(19)).unwrap();assert_eq!(payload.last_ineligible_end,0);
        assert_eq!(view.get(0).unwrap().no_merge_with_previous,false);
        drop(view);drop(payload);assert_eq!(ledger.used(),0);
        eprintln!("CARRY_BOUNDARY guards at first/middle/last chunks;128positions each; room/range translation, mutable tail/COW/truncate; final=0");
    }
    #[test]
    fn subranges_and_tail_share_unchanged_payload_and_release_last_owner() {
        let ledger = Ledger::new();
        let admit = |n| ledger.reserve(n);
        let input = (0..128).map(fragment).collect::<Vec<_>>();
        let source = SharedFragments::from_values(&input, &admit, 256, &mut 10000, &|| false).unwrap();
        let base = ledger.used();
        let view = source.range(1..127, &mut 10000, &|| false).unwrap();
        assert_eq!(
            ledger.used(),
            base + view.descriptor_bytes(),
            "range allocated new fragment payload"
        );
        for i in 0..126 {
            assert!(std::ptr::eq(view.get(i).unwrap(), source.get(i + 1).unwrap()));
            assert_fragment(view.get(i).unwrap(), &input[i + 1]);
        }
        assert!(view.get(126).is_none());
        drop(view);
        assert_eq!(ledger.used(), base);
        let next = source
            .with_tail(127, &[fragment(999)], &mut 10000, &|| false)
            .unwrap();
        for i in 0..127 {
            assert!(std::ptr::eq(next.get(i).unwrap(), source.get(i).unwrap()));
        }
        assert_fragment(next.get(127).unwrap(), &fragment(999));
        drop(source);
        for i in 0..127 {
            assert_fragment(next.get(i).unwrap(), &input[i]);
        }
        drop(next);
        assert_eq!(ledger.used(), 0);
    }
    #[test]
    fn every_admission_cancel_and_short_quota_preserves_source_and_retries() {
        let ledger = Ledger::new();
        let admit = |n| ledger.reserve(n);
        let input = (0..128).map(fragment).collect::<Vec<_>>();
        let tail = (500..520).map(fragment).collect::<Vec<_>>();
        let source = SharedFragments::from_values(&input, &admit, 256, &mut 10000, &|| false).unwrap();
        let base = ledger.used();
        ledger.calls.set(0);
        let checks = Cell::new(0);
        let mut work = 10000;
        let good = source
            .with_tail(117, &tail, &mut work, &|| {
                checks.set(checks.get() + 1);
                false
            })
            .unwrap();
        let admissions = ledger.calls.get();
        let checkpoints = checks.get();
        let spent = 10000 - work;
        drop(good);
        assert_eq!(ledger.used(), base);
        let retry = || {
            for (i, expected) in input.iter().enumerate() {
                assert_fragment(source.get(i).unwrap(), expected);
            }
            let next = source.with_tail(117, &tail, &mut 10000, &|| false).unwrap();
            for i in 0..117 {
                assert!(std::ptr::eq(next.get(i).unwrap(), source.get(i).unwrap()));
            }
            for (i, expected) in tail.iter().enumerate() {
                assert_fragment(next.get(117 + i).unwrap(), expected);
            }
            assert!(next.get(137).is_none());
            drop(next);
            assert_eq!(ledger.used(), base);
        };
        for at in 0..admissions {
            ledger.calls.set(0);
            ledger.reject.set(at);
            assert!(matches!(
                source.with_tail(117, &tail, &mut 10000, &|| false),
                Err(PixelPartitionError::Admission)
            ));
            assert_eq!(ledger.used(), base);
            ledger.reject.set(usize::MAX);
            retry();
        }
        for at in 0..checkpoints {
            let calls = Cell::new(0);
            let result = source.with_tail(117, &tail, &mut 10000, &|| {
                let n = calls.get();
                calls.set(n + 1);
                n == at
            });
            assert!(matches!(result, Err(PixelPartitionError::Cancelled)));
            assert_eq!(ledger.used(), base);
            retry();
        }
        for quota in 0..spent {
            let mut remaining = quota;
            let result = source.with_tail(117, &tail, &mut remaining, &|| false);
            assert!(matches!(result, Err(PixelPartitionError::Limit)));
            assert_eq!(ledger.used(), base);
            retry();
        }
        ledger.limit.set(base);
        assert!(matches!(
            source.with_tail(117, &tail, &mut 10000, &|| false),
            Err(PixelPartitionError::Admission)
        ));
        assert_eq!(ledger.used(), base);
        ledger.limit.set(16 * 1024 * 1024);
        retry();
        assert!(matches!(
            source.range(0..129, &mut 10000, &|| false),
            Err(PixelPartitionError::Invalid)
        ));
        assert!(matches!(
            source.with_tail(128, &vec![fragment(700); 129], &mut 10000, &|| false),
            Err(PixelPartitionError::Limit)
        ));
        assert_eq!(ledger.used(), base);
        drop(source);
        assert_eq!(ledger.used(), 0);
        eprintln!(
            "shared storage rollback admissions={admissions} cancellations={checkpoints} short_quotas={spent} final=0"
        );
    }
    #[test]
    fn real_partition_snapshots_preserve_bits_across_disjoint_and_overlapping_paints() {
        let ledger = Ledger::new();
        let admit = |n| ledger.reserve(n);
        let mut pixel = super::super::OrderedPixel::new(&admit, 512);
        for i in 0..128 {
            let x = (i % 16) as f64 / 16.;
            let y = (i / 16) as f64 / 16.;
            pixel
                .paint_triangle(
                    [
                        kurbo::Point::new(x, y),
                        kurbo::Point::new(x + 1. / 256., y),
                        kurbo::Point::new(x, y + 1. / 256.),
                    ],
                    fragment(i).owner,
                    &mut 1_000_000,
                    &|| false,
                )
                .unwrap();
        }
        let mut snapshot =
            SharedFragments::from_values(&pixel.regions.values, &admit, 512, &mut 10000, &|| false).unwrap();
        let mut shared_prefixes = Vec::new();
        for (index, points) in [
            [[0.9, 0.9], [0.92, 0.9], [0.9, 0.92]],
            [[0.1, 0.1], [0.35, 0.1], [0.1, 0.35]],
            [[0., 0.], [1., 0.], [0., 1.]],
        ]
        .into_iter()
        .enumerate()
        {
            let old = (0..snapshot.len)
                .map(|i| *snapshot.get(i).unwrap())
                .collect::<Vec<_>>();
            pixel
                .paint_triangle(
                    points.map(|p| kurbo::Point::new(p[0], p[1])),
                    fragment(1000 + index).owner,
                    &mut 1_000_000,
                    &|| false,
                )
                .unwrap();
            for (i, expected) in old.iter().enumerate() {
                assert_fragment(snapshot.get(i).unwrap(), expected);
            }
            let prefix = pixel
                .regions
                .values
                .iter()
                .zip(&old)
                .take_while(|(a, b)| same_fragment(a, b))
                .count();
            let next = snapshot
                .with_tail(prefix, &pixel.regions.values[prefix..], &mut 10000, &|| false)
                .unwrap();
            for i in 0..prefix {
                assert!(std::ptr::eq(next.get(i).unwrap(), snapshot.get(i).unwrap()));
            }
            assert_eq!(next.len, pixel.fragments());
            for (i, actual) in pixel.regions.values.iter().enumerate() {
                assert_fragment(next.get(i).unwrap(), actual);
            }
            shared_prefixes.push(prefix);
            snapshot = next;
        }
        assert_eq!(shared_prefixes[0], 128, "disjoint append lost immutable prefix");
        drop(pixel);
        assert!(snapshot.len > 0);
        assert!(snapshot.get(0).unwrap().has_area);
        drop(snapshot);
        assert_eq!(ledger.used(), 0);
        eprintln!("shared real partition: prefixes={shared_prefixes:?}, exact geometry/fields/tags, final=0");
    }
    fn make_partition<'a>(admit: &'a Admitter<'a>) -> OrderedPixel<'a> {
        let mut pixel = OrderedPixel::new(admit, 512);
        for i in 0..128 {
            let x = (i % 16) as f64 / 16.;
            let y = (i / 16) as f64 / 16.;
            pixel
                .paint_triangle(
                    [
                        kurbo::Point::new(x, y),
                        kurbo::Point::new(x + 1. / 256., y),
                        kurbo::Point::new(x, y + 1. / 256.),
                    ],
                    fragment(i).owner,
                    &mut 1_000_000,
                    &|| false,
                )
                .unwrap();
        }
        pixel
    }
    fn append_disjoint(pixel: &mut OrderedPixel<'_>) {
        pixel
            .paint_triangle(
                [
                    kurbo::Point::new(0.9, 0.9),
                    kurbo::Point::new(0.92, 0.9),
                    kurbo::Point::new(0.9, 0.92),
                ],
                fragment(1000).owner,
                &mut 1_000_000,
                &|| false,
            )
            .unwrap();
    }
    #[test]
    fn context_refuses_foreign_owner_and_wrong_prefix_and_resolves_after_pool_drop() {
        let ledger = Ledger::new();
        let admit = |n| ledger.reserve(n);
        let mut pixel = make_partition(&admit);
        let foreign = make_partition(&admit);
        assert!(pixel.lines.keys == foreign.lines.keys);
        let snapshot = BoundSnapshot::capture(&pixel, &admit, 512, &mut 10000, &|| false).unwrap();
        let base = ledger.used();
        let calls = ledger.calls.get();
        assert!(matches!(
            snapshot.checked_partition_tail(&foreign, 128, &mut 10000, &|| false),
            Err(PixelPartitionError::Invalid)
        ));
        assert_eq!(ledger.used(), base);
        assert_eq!(ledger.calls.get(), calls);
        drop(foreign);
        append_disjoint(&mut pixel);
        let grown = snapshot
            .checked_partition_tail(&pixel, 128, &mut 10000, &|| false)
            .unwrap();
        for i in 0..128 {
            assert!(std::ptr::eq(
                grown.payload.get(i).unwrap(),
                snapshot.payload.get(i).unwrap()
            ));
        }
        pixel
            .paint_triangle(
                [
                    kurbo::Point::new(0., 0.),
                    kurbo::Point::new(1., 0.),
                    kurbo::Point::new(0., 1.),
                ],
                fragment(2000).owner,
                &mut 1_000_000,
                &|| false,
            )
            .unwrap();
        grown
            .context
            .accepts_pool(&pixel.lines, &mut 10000, &|| false)
            .unwrap();
        assert!(matches!(
            grown.checked_partition_tail(&pixel, 1, &mut 10000, &|| false),
            Err(PixelPartitionError::Invalid)
        ));
        let mut bad = fragment(10);
        bad.polygon.incoming[0] = u32::MAX;
        assert!(matches!(
            grown.context.validate_tags(&[bad], &mut 10000, &|| false),
            Err(PixelPartitionError::Invalid)
        ));
        assert!(grown.context.resolves(0).is_none());
        let probe = kurbo::Point::new(0.123, 0.456);
        let mut evaluations = Vec::new();
        for i in 0..grown.payload.len {
            let f = grown.payload.get(i).unwrap();
            for v in 0..f.polygon.len {
                let id = f.polygon.incoming[v];
                if id == 0 {
                    continue;
                }
                let key = grown.context.resolves(id).unwrap();
                assert!(key == pixel.lines.keys[id as usize - 1]);
                let a = f.polygon.points[(v + f.polygon.len - 1) % f.polygon.len];
                let b = f.polygon.points[v];
                let (evaluate, _) = key.evaluator(a, b).unwrap();
                evaluations.push((id, a, b, evaluate(probe).to_bits()));
            }
        }
        assert!(!evaluations.is_empty());
        drop(pixel);
        drop(snapshot);
        for (id, a, b, expected) in &evaluations {
            let (evaluate, _) = grown.context.resolves(*id).unwrap().evaluator(*a, *b).unwrap();
            assert_eq!(evaluate(probe).to_bits(), *expected);
        }
        drop(grown);
        assert_eq!(ledger.used(), 0);
        eprintln!(
            "bound context: foreign owner/prefix/tag refused; {} line evaluations survive original pool drop; final=0",
            evaluations.len()
        );
    }
    #[test]
    fn context_detects_rolled_back_id_reuse_without_losing_original_line() {
        let ledger = Ledger::new();
        let admit = |n| ledger.reserve(n);
        let mut pool = LinePool::new(&admit);
        let old_key = LineKey::Axis(0, 0.25f64.to_bits());
        let new_key = LineKey::Axis(1, 0.75f64.to_bits());
        let mut transaction = pool.transaction();
        let id = transaction.intern(old_key, &mut 10000, &|| false).unwrap();
        let captured = LineContext::capture(transaction.pool, &admit, &mut 10000, &|| false).unwrap();
        drop(transaction);
        let mut transaction = pool.transaction();
        let reused = transaction.intern(new_key, &mut 10000, &|| false).unwrap();
        transaction.commit();
        assert_eq!(id, reused);
        assert_eq!(captured.identity, pool.identity);
        assert!(captured.resolves(id) == Some(old_key));
        assert!(pool.keys[id as usize - 1] == new_key);
        assert!(matches!(
            captured.accepts_pool(&pool, &mut 10000, &|| false),
            Err(PixelPartitionError::Invalid)
        ));
        drop(pool);
        assert!(captured.resolves(id) == Some(old_key));
        drop(captured);
        assert_eq!(ledger.used(), 0);
    }
    #[test]
    fn bound_context_every_failure_preserves_previous_snapshot_and_retries() {
        let ledger = Ledger::new();
        let admit = |n| ledger.reserve(n);
        let mut pixel = make_partition(&admit);
        let snapshot = BoundSnapshot::capture(&pixel, &admit, 512, &mut 10000, &|| false).unwrap();
        append_disjoint(&mut pixel);
        let base = ledger.used();
        ledger.calls.set(0);
        let checks = Cell::new(0);
        let mut work = 10000;
        let next = snapshot
            .checked_partition_tail(&pixel, 128, &mut work, &|| {
                checks.set(checks.get() + 1);
                false
            })
            .unwrap();
        let admissions = ledger.calls.get();
        let checkpoints = checks.get();
        let spent = 10000 - work;
        drop(next);
        assert_eq!(ledger.used(), base);
        let retry = || {
            let next = snapshot
                .checked_partition_tail(&pixel, 128, &mut 10000, &|| false)
                .unwrap();
            for i in 0..128 {
                assert!(std::ptr::eq(
                    next.payload.get(i).unwrap(),
                    snapshot.payload.get(i).unwrap()
                ));
            }
            for (i, f) in pixel.regions.values.iter().enumerate() {
                assert_fragment(next.payload.get(i).unwrap(), f);
            }
            assert!(snapshot.context.keys == pixel.lines.keys[..snapshot.context.keys.len()]);
            drop(next);
            assert_eq!(ledger.used(), base);
        };
        for at in 0..admissions {
            ledger.calls.set(0);
            ledger.reject.set(at);
            assert!(matches!(
                snapshot.checked_partition_tail(&pixel, 128, &mut 10000, &|| false),
                Err(PixelPartitionError::Admission)
            ));
            assert_eq!(ledger.used(), base);
            ledger.reject.set(usize::MAX);
            retry();
        }
        for at in 0..checkpoints {
            let checks = Cell::new(0);
            assert!(matches!(
                snapshot.checked_partition_tail(&pixel, 128, &mut 10000, &|| {
                    let n = checks.get();
                    checks.set(n + 1);
                    n == at
                }),
                Err(PixelPartitionError::Cancelled)
            ));
            assert_eq!(ledger.used(), base);
            retry();
        }
        for quota in 0..spent {
            let mut remaining = quota;
            assert!(matches!(
                snapshot.checked_partition_tail(&pixel, 128, &mut remaining, &|| false),
                Err(PixelPartitionError::Limit)
            ));
            assert_eq!(ledger.used(), base);
            retry();
        }
        ledger.limit.set(base);
        assert!(matches!(
            snapshot.checked_partition_tail(&pixel, 128, &mut 10000, &|| false),
            Err(PixelPartitionError::Admission)
        ));
        assert_eq!(ledger.used(), base);
        ledger.limit.set(16 * 1024 * 1024);
        retry();
        drop(snapshot);
        drop(pixel);
        assert_eq!(ledger.used(), 0);
        eprintln!(
            "bound context rollback admissions={admissions} cancellations={checkpoints} short_quotas={spent} final=0"
        );
    }
    fn cut_middle(pixel: &mut OrderedPixel<'_>) {
        let x = (42 % 16) as f64 / 16.;
        let y = (42 / 16) as f64 / 16.;
        let d = 1. / 256.;
        pixel
            .paint_triangle(
                [
                    kurbo::Point::new(x + d / 4., y + d / 4.),
                    kurbo::Point::new(x + 3. * d / 8., y + d / 4.),
                    kurbo::Point::new(x + d / 4., y + 3. * d / 8.),
                ],
                fragment(2000).owner,
                &mut 1_000_000,
                &|| false,
            )
            .unwrap();
    }
    fn middle_suffix(pixel: &OrderedPixel<'_>, old: &BoundSnapshot<'_>) -> usize {
        let start = pixel
            .regions
            .values
            .iter()
            .position(|f| same_fragment(f, old.payload.get(43).unwrap()))
            .unwrap();
        for i in 43..128 {
            assert_fragment(&pixel.regions.values[start + i - 43], old.payload.get(i).unwrap());
        }
        start
    }
    #[test]
    fn mixed_snapshot_runs_share_both_sides_of_actual_middle_cut() {
        let ledger = Ledger::new();
        let admit = |n| ledger.reserve(n);
        let mut pixel = make_partition(&admit);
        let old = BoundSnapshot::capture(&pixel, &admit, 512, &mut 10000, &|| false).unwrap();
        cut_middle(&mut pixel);
        let suffix = middle_suffix(&pixel, &old);
        let end = suffix + 85;
        assert!(suffix > 42);
        assert!(end < pixel.fragments());
        let newer = BoundSnapshot::capture(&pixel, &admit, 512, &mut 10000, &|| false).unwrap();
        let pieces = [
            SnapshotPiece::Shared {
                source: &old,
                range: 0..42,
            },
            SnapshotPiece::New(&pixel.regions.values[42..suffix]),
            SnapshotPiece::Shared {
                source: &newer,
                range: suffix..end,
            },
            SnapshotPiece::New(&pixel.regions.values[end..]),
        ];
        let result =
            BoundSnapshot::checked_partition_pieces(&pixel, &pieces, &admit, 512, &mut 10000, &|| false)
                .unwrap();
        for i in 0..42 {
            assert!(std::ptr::eq(
                result.payload.get(i).unwrap(),
                old.payload.get(i).unwrap()
            ));
        }
        for i in suffix..end {
            assert!(std::ptr::eq(
                result.payload.get(i).unwrap(),
                newer.payload.get(i).unwrap()
            ));
        }
        for (i, f) in pixel.regions.values.iter().enumerate() {
            assert_fragment(result.payload.get(i).unwrap(), f);
        }
        let expected = pixel.regions.values.iter().copied().collect::<Vec<_>>();
        drop(pieces);
        drop(old);
        drop(newer);
        drop(pixel);
        for (i, f) in expected.iter().enumerate() {
            assert_fragment(result.payload.get(i).unwrap(), f);
            for id in &f.polygon.incoming[..f.polygon.len] {
                assert!(*id == 0 || result.context.resolves(*id).is_some());
            }
        }
        drop(result);
        assert_eq!(ledger.used(), 0);
        eprintln!(
            "mixed shared runs: head=42 suffix=85 changed_middle={} changed_tail={} exact geometry/fields/tags, final=0",
            suffix - 42,
            expected.len() - end
        );
    }
    #[test]
    fn mixed_runs_reject_invalid_plans_and_all_failures_rollback() {
        let ledger = Ledger::new();
        let admit = |n| ledger.reserve(n);
        let mut pixel = make_partition(&admit);
        let old = BoundSnapshot::capture(&pixel, &admit, 512, &mut 10000, &|| false).unwrap();
        cut_middle(&mut pixel);
        let suffix = middle_suffix(&pixel, &old);
        let end = suffix + 85;
        let pieces = [
            SnapshotPiece::Shared {
                source: &old,
                range: 0..42,
            },
            SnapshotPiece::New(&pixel.regions.values[42..suffix]),
            SnapshotPiece::Shared {
                source: &old,
                range: 43..128,
            },
            SnapshotPiece::New(&pixel.regions.values[end..]),
        ];
        let base = ledger.used();
        ledger.calls.set(0);
        let checks = Cell::new(0);
        let mut work = 10000;
        let result =
            BoundSnapshot::checked_partition_pieces(&pixel, &pieces, &admit, 512, &mut work, &|| {
                checks.set(checks.get() + 1);
                false
            })
            .unwrap();
        let admissions = ledger.calls.get();
        let checkpoints = checks.get();
        let spent = 10000 - work;
        drop(result);
        assert_eq!(ledger.used(), base);
        let retry = || {
            let next =
                BoundSnapshot::checked_partition_pieces(&pixel, &pieces, &admit, 512, &mut 10000, &|| false)
                    .unwrap();
            for (i, f) in pixel.regions.values.iter().enumerate() {
                assert_fragment(next.payload.get(i).unwrap(), f);
            }
            for i in 0..42 {
                assert!(std::ptr::eq(
                    next.payload.get(i).unwrap(),
                    old.payload.get(i).unwrap()
                ));
            }
            for i in 43..128 {
                assert!(std::ptr::eq(
                    next.payload.get(suffix + i - 43).unwrap(),
                    old.payload.get(i).unwrap()
                ));
            }
            drop(next);
            assert_eq!(ledger.used(), base);
        };
        for at in 0..admissions {
            ledger.calls.set(0);
            ledger.reject.set(at);
            assert!(matches!(
                BoundSnapshot::checked_partition_pieces(&pixel, &pieces, &admit, 512, &mut 10000, &|| false),
                Err(PixelPartitionError::Admission)
            ));
            assert_eq!(ledger.used(), base);
            ledger.reject.set(usize::MAX);
            retry();
        }
        for at in 0..checkpoints {
            let checks = Cell::new(0);
            assert!(matches!(
                BoundSnapshot::checked_partition_pieces(&pixel, &pieces, &admit, 512, &mut 10000, &|| {
                    let n = checks.get();
                    checks.set(n + 1);
                    n == at
                }),
                Err(PixelPartitionError::Cancelled)
            ));
            assert_eq!(ledger.used(), base);
            retry();
        }
        for quota in 0..spent {
            let mut remaining = quota;
            assert!(matches!(
                BoundSnapshot::checked_partition_pieces(
                    &pixel,
                    &pieces,
                    &admit,
                    512,
                    &mut remaining,
                    &|| false
                ),
                Err(PixelPartitionError::Limit)
            ));
            assert_eq!(ledger.used(), base);
            retry();
        }
        assert!(matches!(
            BoundSnapshot::checked_partition_pieces(
                &pixel,
                &pieces,
                &admit,
                pixel.fragments() - 1,
                &mut 10000,
                &|| false
            ),
            Err(PixelPartitionError::Limit)
        ));
        let wrong_order = [
            SnapshotPiece::New(&pixel.regions.values[42..suffix]),
            SnapshotPiece::Shared {
                source: &old,
                range: 0..42,
            },
            SnapshotPiece::Shared {
                source: &old,
                range: 43..128,
            },
            SnapshotPiece::New(&pixel.regions.values[end..]),
        ];
        assert!(matches!(
            BoundSnapshot::checked_partition_pieces(&pixel, &wrong_order, &admit, 512, &mut 10000, &|| false),
            Err(PixelPartitionError::Invalid)
        ));
        let bad_range = [SnapshotPiece::Shared {
            source: &old,
            range: 0..129,
        }];
        assert!(matches!(
            BoundSnapshot::checked_partition_pieces(&pixel, &bad_range, &admit, 512, &mut 10000, &|| false),
            Err(PixelPartitionError::Invalid)
        ));
        assert!(matches!(
            BoundSnapshot::checked_partition_pieces(&pixel, &pieces[..3], &admit, 512, &mut 10000, &|| false),
            Err(PixelPartitionError::Invalid)
        ));
        let foreign_pixel = make_partition(&admit);
        let foreign = BoundSnapshot::capture(&foreign_pixel, &admit, 512, &mut 10000, &|| false).unwrap();
        let foreign_plan = [
            SnapshotPiece::Shared {
                source: &foreign,
                range: 0..42,
            },
            SnapshotPiece::New(&pixel.regions.values[42..suffix]),
            SnapshotPiece::Shared {
                source: &old,
                range: 43..128,
            },
            SnapshotPiece::New(&pixel.regions.values[end..]),
        ];
        assert!(matches!(
            BoundSnapshot::checked_partition_pieces(&pixel, &foreign_plan, &admit, 512, &mut 10000, &|| {
                false
            }),
            Err(PixelPartitionError::Invalid)
        ));
        drop(foreign_plan);
        drop(foreign);
        drop(foreign_pixel);
        assert_eq!(ledger.used(), base);
        retry();
        drop(wrong_order);
        drop(bad_range);
        drop(pieces);
        drop(old);
        drop(pixel);
        assert_eq!(ledger.used(), 0);
        eprintln!(
            "mixed runs rollback admissions={admissions} cancellations={checkpoints} short_quotas={spent}; invalid order/range/count/owner refused; final=0"
        );
    }
    #[test]
    fn partial_chunk_compaction_releases_unused_payload_and_all_failures_rollback() {
        let ledger = Ledger::new();
        let admit = |n| ledger.reserve(n);
        let pixel = make_partition(&admit);
        let source = BoundSnapshot::capture(&pixel, &admit, 512, &mut 10000, &|| false).unwrap();
        let mut payload = SharedFragments::empty(&admit, 512);
        for i in (0..128).step_by(CHUNK_RECORDS) {
            payload
                .append_range(&source.payload, i..i + 1, &mut 10000, &|| false)
                .unwrap();
        }
        let sparse = BoundSnapshot {
            payload,
            context: Arc::clone(&source.context),
        };
        drop(source);
        drop(pixel);
        let before = ledger.used();
        assert_eq!(sparse.payload.len, 8);
        ledger.calls.set(0);
        let checks = Cell::new(0);
        let mut work = 10000;
        let next = sparse
            .compacted(&mut work, &|| {
                checks.set(checks.get() + 1);
                false
            })
            .unwrap();
        let admissions = ledger.calls.get();
        let checkpoints = checks.get();
        let spent = 10000 - work;
        drop(next);
        assert_eq!(ledger.used(), before);
        let retry = || {
            let next = sparse.compacted(&mut 10000, &|| false).unwrap();
            assert!(Arc::ptr_eq(&next.context, &sparse.context));
            assert_eq!(next.payload.len, 8);
            for i in 0..8 {
                assert_fragment(next.payload.get(i).unwrap(), sparse.payload.get(i).unwrap());
                assert!(!std::ptr::eq(
                    next.payload.get(i).unwrap(),
                    sparse.payload.get(i).unwrap()
                ));
            }
            drop(next);
            assert_eq!(ledger.used(), before);
        };
        for at in 0..admissions {
            ledger.calls.set(0);
            ledger.reject.set(at);
            assert!(matches!(
                sparse.compacted(&mut 10000, &|| false),
                Err(PixelPartitionError::Admission)
            ));
            assert_eq!(ledger.used(), before);
            ledger.reject.set(usize::MAX);
            retry();
        }
        for at in 0..checkpoints {
            let checks = Cell::new(0);
            assert!(matches!(
                sparse.compacted(&mut 10000, &|| {
                    let n = checks.get();
                    checks.set(n + 1);
                    n == at
                }),
                Err(PixelPartitionError::Cancelled)
            ));
            assert_eq!(ledger.used(), before);
            retry();
        }
        for quota in 0..spent {
            let mut remaining = quota;
            assert!(matches!(
                sparse.compacted(&mut remaining, &|| false),
                Err(PixelPartitionError::Limit)
            ));
            assert_eq!(ledger.used(), before);
            retry();
        }
        ledger.limit.set(before);
        assert!(matches!(
            sparse.compacted(&mut 10000, &|| false),
            Err(PixelPartitionError::Admission)
        ));
        assert_eq!(ledger.used(), before);
        ledger.limit.set(16 * 1024 * 1024);
        retry();
        let compact = sparse.compacted(&mut 10000, &|| false).unwrap();
        drop(sparse);
        let after = ledger.used();
        assert!(after < before);
        for i in 0..compact.payload.len {
            let f = compact.payload.get(i).unwrap();
            for id in &f.polygon.incoming[..f.polygon.len] {
                assert!(*id == 0 || compact.context.resolves(*id).is_some());
            }
        }
        drop(compact);
        assert_eq!(ledger.used(), 0);
        eprintln!(
            "partial chunks: live=8 backing=128 before={before} after={after}; rollback admissions={admissions} cancellations={checkpoints} short_quotas={spent}; exact values/context; final=0"
        );
    }
    #[test]
    fn empty_compaction_shares_context_without_new_admission() {
        let ledger = Ledger::new();
        let admit = |n| ledger.reserve(n);
        let pixel = OrderedPixel::new(&admit, 0);
        let empty = BoundSnapshot::capture(&pixel, &admit, 0, &mut 10000, &|| false).unwrap();
        drop(pixel);
        let base = ledger.used();
        ledger.calls.set(0);
        ledger.limit.set(base);
        let next = empty.compacted(&mut 10000, &|| false).unwrap();
        assert_eq!(next.payload.len, 0);
        assert!(next.payload.get(0).is_none());
        assert!(Arc::ptr_eq(&empty.context, &next.context));
        assert_eq!(ledger.calls.get(), 0);
        assert_eq!(ledger.used(), base);
        drop(next);
        for quota in 0..2 {
            let mut remaining = quota;
            assert!(matches!(
                empty.compacted(&mut remaining, &|| false),
                Err(PixelPartitionError::Limit)
            ));
            assert_eq!(ledger.used(), base);
        }
        for at in 0..2 {
            let checks = Cell::new(0);
            assert!(matches!(
                empty.compacted(&mut 10000, &|| {
                    let n = checks.get();
                    checks.set(n + 1);
                    n == at
                }),
                Err(PixelPartitionError::Cancelled)
            ));
            assert_eq!(ledger.used(), base);
        }
        drop(empty);
        assert_eq!(ledger.used(), 0);
    }
    #[test]
    fn oriented_chunk_bounds_prune_diagonal_gaps_and_keep_ambiguous_overlap() {
        let mut values = Vec::new();
        for i in 0..16 {
            let mut item = fragment(i);
            item.polygon = super::super::Polygon::empty();
            let shift = i as f64 / 1000.;
            for [x, y] in [[0.1, 0.1 + shift], [0.9, 0.7 + shift], [0.9, 0.71 + shift]] {
                item.polygon.push(kurbo::Point::new(x, y)).unwrap();
            }
            // Distinct synthetic support labels suffice for this metadata
            // predicate; real LinePool bindings are covered by kernel replays.
            for edge in 0..item.polygon.len { item.polygon.incoming[edge] = (i*3+edge+1) as u32; }
            item.no_merge_with_previous = true;
            item = Fragment::new(item.polygon,item.owner,item.has_area,item.no_merge_with_previous);
            values.push(item);
        }
        let summary = ChunkSummary::from_values(&values);
        assert!(summary.eligible && summary.axis != [0.; 2]);
        let mut cut = super::super::Polygon::empty();
        for [x, y] in [[0.1, 0.2], [0.8, 0.725], [0.8, 0.735]] {
            cut.push(kurbo::Point::new(x, y)).unwrap();
        }
        for edge in 0..cut.len { cut.incoming[edge] = (100+edge) as u32; }
        // Axis-aligned bounds overlap; the additional oriented interval is
        // needed to prove the finite figures separate.
        assert!(summary.bounds[0] < 0.8 && summary.bounds[2] > 0.1);
        assert!(summary.bounds[1] < 0.735 && summary.bounds[3] > 0.2);
        let checks = Cell::new(0);
        let mut work = 100;
        assert_eq!(
            summary.disjoint(&cut, &mut work, &|| {
                checks.set(checks.get() + 1);
                false
            }),
            Ok(true)
        );
        let cost = 100 - work;
        assert_eq!(cost, 3);
        for stop in 0..checks.get() {
            let at = Cell::new(0);
            assert_eq!(
                summary.disjoint(&cut, &mut 100, &|| {
                    let now = at.get();
                    at.set(now + 1);
                    now == stop
                }),
                Err(PixelPartitionError::Cancelled)
            );
        }
        for mut quota in 0..cost {
            assert_eq!(
                summary.disjoint(&cut, &mut quota, &|| false),
                Err(PixelPartitionError::Limit)
            );
        }
        assert_eq!(
            summary.disjoint(&values[0].polygon, &mut 100, &|| false),
            Ok(false)
        );
        let mut unknown = summary;
        unknown.known_edges = false;
        assert_eq!(unknown.disjoint(&cut, &mut 100, &|| false), Ok(false));
        let mut unknown_cut = cut;
        unknown_cut.incoming[0] = 0;
        assert_eq!(summary.disjoint(&unknown_cut, &mut 100, &|| false), Ok(false));
        let mut uncertain = summary;
        uncertain.projection = [f64::NEG_INFINITY, f64::INFINITY];
        assert_eq!(uncertain.disjoint(&cut, &mut 100, &|| false), Ok(false));
        let admit = |_: usize| Some(Box::new(()) as Credit);
        let mut output = mode_pixel(&admit, 4096, true);
        for item in &values {
            output.regions.sink_push(*item).unwrap();
        }
        assert_eq!(
            output
                .regions
                .carry_span(0, &cut, false, 4096, &mut 100, &|| false)
                .unwrap(),
            Some((16, true))
        );
    }
    #[test]
    fn oriented_projection_intervals_cover_subnormal_cancellation_and_overflow() {
        let minimum = f64::from_bits(1);
        let positive = ChunkSummary::project([0.5, 0.], kurbo::Point::new(minimum, 0.)).unwrap();
        assert!(positive[0] <= 0. && positive[1] >= minimum);
        let negative = ChunkSummary::project([-0.5, 0.], kurbo::Point::new(minimum, 0.)).unwrap();
        assert!(negative[0] <= -minimum && negative[1] >= 0.);
        let cancelled = ChunkSummary::project([1., -1.], kurbo::Point::new(1., 1.)).unwrap();
        assert!(cancelled[0] < 0. && cancelled[1] > 0.);
        let exact = ChunkSummary::project([0.5, -0.25], kurbo::Point::new(0.5, 0.5)).unwrap();
        assert!(exact[0] <= 0.125 && exact[1] >= 0.125);
        assert!(ChunkSummary::project([f64::MAX, f64::MAX], kurbo::Point::new(f64::MAX, -f64::MAX)).is_none());
    }
    #[test]
    #[ignore = "manual boundary-cancellation eligibility investigation; not a production fast path"]
    fn captured_boundary_cancellation_probe() {
        use std::collections::BTreeMap;
        type Row = (usize, usize, usize, [[u64; 2]; 3], [[u64; 3]; 6]);
        let (dense, _): (&[Row], usize) = include!("../../../../tests/fixtures/geometry/dense-cell-176.rs");
        let (coverage82, _): (&[Row], usize) =
            include!("../../../../tests/fixtures/geometry/coverage-cell-82.rs");
        let (coverage103, _): (&[Row], usize) =
            include!("../../../../tests/fixtures/geometry/coverage-cell-103.rs");
        for (name, rows) in [
            ("dense1236", dense),
            ("coverage116", coverage82),
            ("coverage328", coverage103),
        ] {
            let mut edges = BTreeMap::new();
            let mut integral = [0.; 5];
            let mut work = 100_000_000;
            for &(_, _, count, points, fields) in rows {
                let mut points = points.map(|p| kurbo::Point::new(f64::from_bits(p[0]), f64::from_bits(p[1])));
                if super::super::orientation(points[0], points[1], points[2]) < 0 {
                    points.swap(1, 2);
                }
                let mut polygon = super::super::Polygon::empty();
                for p in points {
                    polygon.push(p).unwrap();
                }
                for line in [[0., 1., 0.], [1., -1., 0.], [0., 0., 1.], [1., 0., -1.]] {
                    polygon = polygon.clip(line, &mut work, &|| false).unwrap();
                }
                let Some((area, center)) = polygon.moments() else {
                    continue;
                };
                integral[0] += area;
                for k in 0..count {
                    let f = fields[k + 2].map(f64::from_bits);
                    integral[k + 1] += area * f[1].mul_add(center.x, f[2].mul_add(center.y, f[0]));
                }
                let bits = |p: kurbo::Point| {
                    assert!(p.x >= 0. && p.x <= 1. && p.y >= 0. && p.y <= 1.);
                    [p.x, p.y].map(|v| if v == 0. { 0 } else { v.to_bits() })
                };
                for [a, b] in polygon.edges() {
                    let (a, b) = (bits(a), bits(b));
                    if a == b {
                        continue;
                    }
                    let (key, sign) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
                    *edges.entry(key).or_insert(0i32) += sign;
                }
            }
            let mut interior = 0;
            let mut invalid = 0;
            let mut last = [0u64; 4];
            let mut residual = 0;
            for ((a, b), count) in edges {
                if count == 0 {
                    continue;
                }
                residual += 1;
                let side = if a[1] == 0 && b[1] == 0 {
                    Some((0, a[0], b[0], 1))
                } else if a[0] == 1f64.to_bits() && b[0] == 1f64.to_bits() {
                    Some((1, a[1], b[1], 1))
                } else if a[1] == 1f64.to_bits() && b[1] == 1f64.to_bits() {
                    Some((2, a[0], b[0], -1))
                } else if a[0] == 0 && b[0] == 0 {
                    Some((3, a[1], b[1], -1))
                } else {
                    None
                };
                if let Some((side, from, to, wanted)) = side {
                    if count != wanted || from != last[side] {
                        invalid += 1;
                    }
                    last[side] = to;
                } else {
                    interior += 1;
                }
            }
            let full = interior == 0 && invalid == 0 && last == [1f64.to_bits(); 4];
            eprintln!(
                "BOUNDARY_CERTIFICATE name={name} full={full} residual={residual} interior={interior} invalid_border_intervals={invalid} integral={integral:?} work={}",
                100_000_000 - work
            );
        }
    }
    fn occluded_suffix_seed<'a>(admit: &'a Admitter<'a>) -> OrderedPixel<'a> {
        let mut pixel=mode_pixel(admit,2048,true);
        for i in 0..1024 {
            let a=i as f64/1024.;let b=(i+1) as f64/1024.;
            let owner=super::super::PixelSourceOwner {patch_index:i+1,sample_index:0,component_count:1,
                u:[0.;3],v:[0.;3],components:[[0.25,0.,0.],[0.;3],[0.;3],[0.;3]]};
            for points in [[kurbo::Point::new(a,0.),kurbo::Point::new(b,0.),kurbo::Point::new(b,1.)],
                [kurbo::Point::new(a,0.),kurbo::Point::new(b,1.),kurbo::Point::new(a,1.)]] {
                pixel.paint_triangle(points,owner,&mut 100_000_000,&||false).unwrap();
            }
        }
        assert_eq!(pixel.fragments(),1024);pixel
    }
    #[test]
    #[ignore = "manual closed-input CPU timing; input construction is excluded"]
    fn occluded_suffix_fixed_work_cpu() {
        use std::{hint::black_box,time::Instant};
        let repetitions=std::env::var("RRRAH_CELL_CPU_REPETITIONS").ok().map(|s|s.parse::<usize>().unwrap()).unwrap_or(8);
        assert!((1..=256).contains(&repetitions));
        let admit=|_:usize|Some(Box::new(()) as Credit);
        let owner=super::super::PixelSourceOwner {patch_index:0,sample_index:0,component_count:1,
            u:[0.;3],v:[0.;3],components:[[0.75,0.,0.],[0.;3],[0.;3],[0.;3]]};
        let points=[kurbo::Point::new(1./4096.,0.25),kurbo::Point::new(2./4096.,0.25),kurbo::Point::new(1./4096.,0.75)];
        let mut elapsed_ns=0;let mut total_work=0;
        for _ in 0..repetitions {
            let mut pixel=occluded_suffix_seed(&admit);let mut work=100_000_000;
            let start=Instant::now();
            pixel.paint_triangle(black_box(points),black_box(owner),&mut work,&||false).unwrap();
            elapsed_ns+=start.elapsed().as_nanos();total_work+=100_000_000-work;
            assert_eq!(pixel.fragments(),1024);let mut area=0.;
            pixel.visit(&mut 100_000_000,&||false,&mut |a,_,_,owner| {
                assert!(owner.patch_index>0);assert_eq!(owner.components[0],[0.25,0.,0.]);area+=a;Ok(())
            }).unwrap();assert_eq!(area,1.);black_box(pixel.fragments());
        }
        println!("DENSE_FIXED occluded1024 elapsed_ns={elapsed_ns} repetitions={repetitions} total_work={total_work}");
    }
    fn guarded_suffix_seed<'a>(admit: &'a Admitter<'a>) -> OrderedPixel<'a> {
        let mut pixel=occluded_suffix_seed(admit);let owner=pixel.regions.values[31].owner;
        let mut rebuilt=pixel.regions.empty_like();
        for (i,old) in pixel.regions.values.iter().enumerate() {
            let mut item=*old;if i==32 {item.owner=owner;item.no_merge_with_previous=false;}
            rebuilt.sink_push(item).unwrap();
        }
        pixel.regions=rebuilt;pixel
    }
    #[test]
    #[ignore = "manual guarded-suffix CPU timing; seed construction excluded"]
    fn guarded_suffix_fixed_work_cpu() {
        use std::{hint::black_box,time::Instant};
        let repetitions=std::env::var("RRRAH_CELL_CPU_REPETITIONS").ok().map(|s|s.parse::<usize>().unwrap()).unwrap_or(8);
        assert!((1..=256).contains(&repetitions));let admit=|_:usize|Some(Box::new(()) as Credit);
        let owner=super::super::PixelSourceOwner {patch_index:0,sample_index:0,component_count:1,
            u:[0.;3],v:[0.;3],components:[[0.75,0.,0.],[0.;3],[0.;3],[0.;3]]};
        let points=[kurbo::Point::new(1./4096.,0.25),kurbo::Point::new(2./4096.,0.25),kurbo::Point::new(1./4096.,0.75)];
        let mut elapsed_ns=0;let mut total_work=0;
        for _ in 0..repetitions {
            let mut pixel=guarded_suffix_seed(&admit);let mut work=100_000_000;let start=Instant::now();
            pixel.paint_triangle(black_box(points),black_box(owner),&mut work,&||false).unwrap();
            elapsed_ns+=start.elapsed().as_nanos();total_work+=100_000_000-work;
            assert_eq!(pixel.fragments(),1023);let mut area=0.;
            pixel.visit(&mut 100_000_000,&||false,&mut |a,_,_,owner| {
                assert!(owner.patch_index>0);assert_eq!(owner.components[0],[0.25,0.,0.]);area+=a;Ok(())
            }).unwrap();assert_eq!(area,1.);black_box(pixel.fragments());
        }
        println!("DENSE_FIXED guarded1024 elapsed_ns={elapsed_ns} repetitions={repetitions} total_work={total_work}");
    }
    #[test]
    fn native_guarded_suffix_preserves_a_required_merge_and_all_line_ids() {
        let admit=|_:usize|Some(Box::new(()) as Credit);
        let mut fast=guarded_suffix_seed(&admit);let mut reference=guarded_suffix_seed(&admit);
        let RegionValues::Shared(values)=&fast.regions.values else {panic!()};assert_eq!(values.last_ineligible_end,48);
        let owner=super::super::PixelSourceOwner {patch_index:0,sample_index:0,component_count:1,u:[0.;3],v:[0.;3],components:[[0.75,0.,0.],[0.;3],[0.;3],[0.;3]]};
        let points=[kurbo::Point::new(1./4096.,0.25),kurbo::Point::new(2./4096.,0.25),kurbo::Point::new(1./4096.,0.75)];
        let mut a=100_000_000;let mut b=a;
        fast.paint_triangle_impl::<true>(points,owner,&mut a,&||false).unwrap();reference.paint_triangle_impl::<false>(points,owner,&mut b,&||false).unwrap();
        assert_eq!(fast.fragments(),1023);assert_eq!(reference.fragments(),1023);
        assert!(fast.lines.keys==reference.lines.keys && fast.lines.map==reference.lines.map);assert_eq!(fast.lines.next,reference.lines.next);
        for (a,b) in fast.regions.values.iter().zip(reference.regions.values.iter()) {assert_fragment(a,b);}
        assert_summaries(&fast);assert_summaries(&reference);
        eprintln!("GUARDED_SUFFIX actual convex merge retained;1023regions exact fields/tags/full line context; batch_work={} unbatched_work={}",100_000_000-a,100_000_000-b);
    }
    #[test]
    fn native_occluded_seed_has_a_complete_eligible_suffix() {
        let admit=|_:usize|Some(Box::new(()) as Credit);let pixel=occluded_suffix_seed(&admit);
        let RegionValues::Shared(values)=&pixel.regions.values else {panic!()};
        assert_eq!(values.last_ineligible_end,0);assert_summaries(&pixel);
        assert_eq!(pixel.regions.carry_span(1,&super::super::Polygon::empty(),true,2047,&mut 100,&||false).unwrap(),Some((1024,true)));
    }
    #[test]
    #[ignore = "manual fixed-input CPU timing; no page/display inference"]
    fn captured_dense_cell_fixed_work_cpu() {
        use std::{hint::black_box, time::Instant};
        type Row = (usize, usize, usize, [[u64; 2]; 3], [[u64; 3]; 6]);
        let (rows, _): (&[Row], usize) = include!("../../../../tests/fixtures/geometry/dense-cell-176.rs");
        let repetitions = std::env::var("RRRAH_CELL_CPU_REPETITIONS")
            .ok()
            .map(|v| v.parse::<usize>().unwrap())
            .unwrap_or(8);
        assert!((1..=256).contains(&repetitions));
        let admit = |_: usize| Some(Box::new(()) as Credit);
        let mut total_work = 0;
        let start = Instant::now();
        for _ in 0..repetitions {
            let mut pixel = mode_pixel(&admit, 4096, true);
            let mut work = 100_000_000;
            for &(patch_index, sample_index, component_count, points, fields) in rows {
                let f = fields.map(|v| v.map(f64::from_bits));
                let owner = super::super::PixelSourceOwner {
                    patch_index,
                    sample_index,
                    component_count,
                    u: f[0],
                    v: f[1],
                    components: [f[2], f[3], f[4], f[5]],
                };
                let points = points.map(|p| kurbo::Point::new(f64::from_bits(p[0]), f64::from_bits(p[1])));
                pixel
                    .paint_triangle(black_box(points), black_box(owner), &mut work, &|| false)
                    .unwrap();
            }
            let mut area = 0.;
            pixel
                .visit(&mut work, &|| false, &mut |a, _, _, _| {
                    area += a;
                    Ok(())
                })
                .unwrap();
            assert!((area - 1.).abs() < 1e-10);
            black_box((pixel.fragments(), area));
            total_work += 100_000_000 - work;
        }
        let elapsed_ns = start.elapsed().as_nanos();
        println!(
            "DENSE_FIXED dense1236 elapsed_ns={elapsed_ns} repetitions={repetitions} total_work={total_work}"
        );
    }
    fn assert_summaries(pixel: &OrderedPixel<'_>) {
        if let RegionValues::Shared(values) = &pixel.regions.values {
            assert!(values.last_ineligible_end <= values.len);
            for segment in &values.segments.values {
                if segment.end > values.last_ineligible_end { assert!(segment.chunk.summary.eligible); }
                let expected = ChunkSummary::from_values(&segment.chunk.values);
                assert_eq!(
                    expected.bounds.map(f64::to_bits),
                    segment.chunk.summary.bounds.map(f64::to_bits)
                );
                assert_eq!(expected.axis.map(f64::to_bits), segment.chunk.summary.axis.map(f64::to_bits));
                assert_eq!(expected.projection.map(f64::to_bits), segment.chunk.summary.projection.map(f64::to_bits));
                assert_eq!(expected.known_edges, segment.chunk.summary.known_edges);
                assert_eq!(expected.eligible, segment.chunk.summary.eligible);
                assert!(segment.start + segment.len <= segment.chunk.values.len());
            }
        }
    }
    fn mode_pixel<'a>(admit: &'a Admitter<'a>, maximum: usize, shared: bool) -> OrderedPixel<'a> {
        let previous = set_shared(shared);
        let pixel = OrderedPixel::new(admit, maximum);
        set_shared(previous);
        assert_eq!(matches!(&pixel.regions.values, RegionValues::Shared(_)), shared);
        pixel
    }
    #[test]
    fn actual_shared_kernel_matches_flat_dense_prefix_bits_and_line_ids() {
        type Row = (usize, usize, usize, [[u64; 2]; 3], [[u64; 3]; 6]);
        let (fifty, _): (&[Row], u64) = include!("../../../../tests/fixtures/geometry/dense-lineage.rs");
        let (seventy_five, _): (&[Row], u64) =
            include!("../../../../tests/fixtures/geometry/dense-lineage-138.rs");
        let admit = |_: usize| Some(Box::new(()) as Credit);
        for rows in [fifty, seventy_five] {
            let mut flat = mode_pixel(&admit, 4096, false);
            let mut shared = mode_pixel(&admit, 4096, true);
            assert!(matches!(shared.regions.values, RegionValues::Shared(_)));
            let mut flat_work = 100_000_000;
            let mut shared_work = flat_work;
            for (step, &(patch, sample, count, points, fields)) in rows.iter().enumerate() {
                let points = points.map(|p| kurbo::Point::new(f64::from_bits(p[0]), f64::from_bits(p[1])));
                let f = fields.map(|f| f.map(f64::from_bits));
                let owner = super::super::PixelSourceOwner {
                    patch_index: patch,
                    sample_index: sample,
                    component_count: count,
                    u: f[0],
                    v: f[1],
                    components: [f[2], f[3], f[4], f[5]],
                };
                let old = flat.paint_triangle(points, owner, &mut flat_work, &|| false);
                let new = shared.paint_triangle(points, owner, &mut shared_work, &|| false);
                assert_eq!(old, new);
                assert_summaries(&shared);
                assert_eq!(flat.fragments(), shared.fragments());
                for (a, b) in flat.regions.values.iter().zip(shared.regions.values.iter()) {
                    assert_fragment(a, b);
                }
                if flat.lines.keys != shared.lines.keys {
                    let describe = |key: &LineKey| match key {
                        LineKey::Edge(bits) => format!("edge {bits:?}"),
                        LineKey::Field(bits) => format!("field {bits:?}"),
                        LineKey::Axis(axis,bits) => format!("axis {axis} {bits}"),
                    };
                    let common=flat.lines.keys.iter().zip(&shared.lines.keys).take_while(|(a,b)| a==b).count();
                    eprintln!("LINEAGE_DIVERGENCE rows={} step={step} patch={patch} common={common} flat={} shared={} flat_next={} shared_next={}",rows.len(),flat.lines.keys.len(),shared.lines.keys.len(),flat.lines.next,shared.lines.next);
                    for key in &flat.lines.keys[common..] {eprintln!("FLAT_EXTRA {}",describe(key));}
                    for key in &shared.lines.keys[common..] {eprintln!("SHARED_EXTRA {}",describe(key));}
                }
                assert!(flat.lines.keys == shared.lines.keys, "rows={} step={step}",rows.len());
                assert_eq!(flat.lines.next, shared.lines.next);
            }
            eprintln!(
                "actual shared kernel rows={} flat_work={} shared_work={} exact prefixes/line IDs",
                rows.len(),
                100_000_000 - flat_work,
                100_000_000 - shared_work
            );
        }
    }
    fn populate(pixel: &mut OrderedPixel<'_>) {
        for i in 0..128 {
            let x = (i % 16) as f64 / 16.;
            let y = (i / 16) as f64 / 16.;
            pixel
                .paint_triangle(
                    [
                        kurbo::Point::new(x, y),
                        kurbo::Point::new(x + 1. / 256., y),
                        kurbo::Point::new(x, y + 1. / 256.),
                    ],
                    fragment(i).owner,
                    &mut 1_000_000,
                    &|| false,
                )
                .unwrap();
        }
    }
    #[test]
    fn shared_chunk_proofs_save_sparse_work_and_survive_mixed_overlap_prefixes() {
        let admit = |_: usize| Some(Box::new(()) as Credit);
        let mut flat = mode_pixel(&admit, 4096, false);
        let mut shared = mode_pixel(&admit, 4096, true);
        let mut flat_work = 100_000_000;
        let mut shared_work = flat_work;
        let compare = |a: &OrderedPixel<'_>, b: &OrderedPixel<'_>| {
            assert_eq!(a.fragments(), b.fragments());
            assert!(a.lines.keys == b.lines.keys);
            assert_eq!(a.lines.next, b.lines.next);
            for (a, b) in a.regions.values.iter().zip(b.regions.values.iter()) {
                assert_fragment(a, b);
            }
            assert_summaries(b);
        };
        for i in 0..256 {
            let x = (i % 16) as f64 / 16.;
            let y = (i / 16) as f64 / 16.;
            let points = [
                kurbo::Point::new(x, y),
                kurbo::Point::new(x + 1. / 256., y),
                kurbo::Point::new(x, y + 1. / 256.),
            ];
            flat.paint_triangle(points, fragment(i).owner, &mut flat_work, &|| false)
                .unwrap();
            shared
                .paint_triangle(points, fragment(i).owner, &mut shared_work, &|| false)
                .unwrap();
            compare(&flat, &shared);
        }
        assert!(shared_work > flat_work, "no whole-block work reduction");
        let mut area = 0.;
        shared
            .visit(&mut 1_000_000, &|| false, &mut |a, _, _, _| {
                area += a;
                Ok(())
            })
            .unwrap();
        assert_eq!(area, 1. / 512.);
        eprintln!(
            "shared chunk sparse256 flat_work={} shared_work={} exact area={area}",
            100_000_000 - flat_work,
            100_000_000 - shared_work
        );
        let mut state = 0x71a9bc33u64;
        for i in 0..128 {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            let x = (next() % 128) as f64 / 256.;
            let y = (next() % 128) as f64 / 256.;
            let d = [1. / 64., 1. / 8., 1. / 2.][(next() % 3) as usize];
            let points = [
                kurbo::Point::new(x, y),
                kurbo::Point::new(x + d, y),
                kurbo::Point::new(x, y + d),
            ];
            let owner = fragment(if i % 3 == 0 { i % 17 } else { 1000 + i }).owner;
            let a = flat.paint_triangle(points, owner, &mut 10_000_000, &|| false);
            let b = shared.paint_triangle(points, owner, &mut 10_000_000, &|| false);
            assert_eq!(a, b, "mixed paint {i}");
            compare(&flat, &shared);
        }
        eprintln!(
            "shared chunk mixed paints=128: exact geometry/affine/tag/line-ID prefixes and cached proofs"
        );
    }
    #[test]
    fn captured_dense_lineage_matches_independent_rational_union() {
        type Row = (usize, usize, usize, [[u64; 2]; 3], [[u64; 3]; 6]);
        let (rows, expected): (&[Row], u64) = include!("../../../../tests/fixtures/geometry/dense-lineage.rs");
        let admit = |_: usize| Some(Box::new(()) as Credit);
        for shared in [false, true] {
            let mut pixel = mode_pixel(&admit, 4096, shared);
            for &(patch_index, sample_index, component_count, points, fields) in rows {
                let f = fields.map(|v| v.map(f64::from_bits));
                let owner = super::super::PixelSourceOwner {
                    patch_index,
                    sample_index,
                    component_count,
                    u: f[0],
                    v: f[1],
                    components: [f[2], f[3], f[4], f[5]],
                };
                let points = points.map(|p| kurbo::Point::new(f64::from_bits(p[0]), f64::from_bits(p[1])));
                pixel
                    .paint_triangle(points, owner, &mut 100_000_000, &|| false)
                    .unwrap();
            }
            let mut area = 0.;
            pixel
                .visit(&mut 100_000_000, &|| false, &mut |a, _, _, _| {
                    area += a;
                    Ok(())
                })
                .unwrap();
            assert!(
                (area - f64::from_bits(expected)).abs() < 1e-13,
                "shared={shared} area={area} expected={}",
                f64::from_bits(expected)
            );
        }
    }
    #[test]
    fn captured_coverage_cell_stays_inside_unit_partition() {
        type Row = (usize, usize, usize, [[u64; 2]; 3], [[u64; 3]; 6]);
        let (first, _): (&[Row], usize) = include!("../../../../tests/fixtures/geometry/coverage-cell-82.rs");
        let (second, _): (&[Row], usize) = include!("../../../../tests/fixtures/geometry/coverage-cell-103.rs");
        assert_eq!(first.len(), 116);
        assert_eq!(second.len(), 328);
        for (rows, expected_bits) in [(first, 4607182418800017408u64), (second, 4607182411973925879u64)] {
            let admit = |_: usize| Some(Box::new(()) as Credit);
            for shared in [false, true] {
                let mut pixel = mode_pixel(&admit, 4096, shared);
                let mut area = 0.;
                for (index, &(patch_index, sample_index, component_count, points, fields)) in
                    rows.iter().enumerate()
                {
                    let f = fields.map(|v| v.map(f64::from_bits));
                    let owner = super::super::PixelSourceOwner {
                        patch_index,
                        sample_index,
                        component_count,
                        u: f[0],
                        v: f[1],
                        components: [f[2], f[3], f[4], f[5]],
                    };
                    let points = points.map(|p| kurbo::Point::new(f64::from_bits(p[0]), f64::from_bits(p[1])));
                    pixel
                        .paint_triangle(points, owner, &mut 100_000_000, &|| false)
                        .unwrap();
                    let hulls: Vec<_> = pixel
                        .regions
                        .values
                        .iter()
                        .map(|f| {
                            let mut polygon = f.polygon;
                            assert!(
                                polygon.points[..polygon.len]
                                    .iter()
                                    .all(|p| p.x >= 0. && p.x <= 1. && p.y >= 0. && p.y <= 1.)
                            );
                            // Test the represented geometry independently of inherited
                            // support tags and near-corner winding/reflex artifacts.
                            polygon.incoming.fill(0);
                            polygon.convex_hull()
                        })
                        .collect();
                    for i in 0..hulls.len() {
                        let a = hulls[i];
                        for j in 0..i {
                            let b = hulls[j];
                            if !a.bounds_overlap(&b) {
                                continue;
                            }
                            // Geometric clipping without support IDs is independent
                            // of the support-direction path being qualified here.
                            let mut overlap = a;
                            for [p, q] in b.edges() {
                                overlap = overlap
                                    .clip_by(
                                        super::super::edge_function(p, q),
                                        super::super::edge_snap(p, q),
                                        &mut 1_000_000,
                                        &|| false,
                                    )
                                    .unwrap();
                                if overlap.moments().is_none() {
                                    break;
                                }
                            }
                            let weight = overlap.moments().map_or(0., |(area, _)| area);
                            assert!(
                                weight <= 1e-10,
                                "shared={shared} paint={index} overlap={i},{j} area={weight}"
                            );
                        }
                    }
                    area = 0.;
                    pixel
                        .visit(&mut 100_000_000, &|| false, &mut |a, _, _, _| {
                            area += a;
                            Ok(())
                        })
                        .unwrap();
                    assert!(area <= 1. + 1e-10, "shared={shared} paint={index} area={area}");
                    if shared {
                        assert_summaries(&pixel);
                    }
                }
                assert!(
                    (area - f64::from_bits(expected_bits)).abs() < 1e-13,
                    "coverage{} shared={shared} area={area} exact={}",
                    rows.len(),
                    f64::from_bits(expected_bits)
                );
                eprintln!(
                    "coverage{} shared={shared} final_area={area}; every prefix bounded/disjoint and unit-contained",
                    rows.len()
                );
            }
        }
    }
    type CapturedOrderRow = (usize, usize, usize, [[u64; 2]; 3], [[u64; 3]; 6]);
    fn vertex_max_order(rows: &[CapturedOrderRow]) -> (Vec<usize>, usize) {
        // Equal UV fields retain a common key and original order, including
        // signed-zero aliases. Geometry and colour never break those ties.
        let group_key = |row: &CapturedOrderRow| {
            (
                row.0,
                [row.4[0], row.4[1]].map(|f| f.map(|b| if f64::from_bits(b) == 0. { 0 } else { b })),
            )
        };
        let mut ranks = std::collections::BTreeMap::new();
        for row in rows {
            let points = row.3.map(|p| p.map(f64::from_bits));
            let uv = [row.4[1], row.4[0]].map(|f| {
                let f = f.map(f64::from_bits);
                points
                    .iter()
                    .map(|p| f[1].mul_add(p[0], f[2].mul_add(p[1], f[0])))
                    .fold(f64::NEG_INFINITY, f64::max)
            });
            let rank = ranks.entry(group_key(row)).or_insert(uv);
            if uv[0]
                .total_cmp(&rank[0])
                .then_with(|| uv[1].total_cmp(&rank[1]))
                .is_gt()
            {
                *rank = uv;
            }
        }
        let mut order = (0..rows.len()).collect::<Vec<_>>();
        order.sort_by(|&a, &b| {
            let x = ranks[&group_key(&rows[a])];
            let y = ranks[&group_key(&rows[b])];
            rows[b]
                .0
                .cmp(&rows[a].0)
                .then_with(|| y[0].total_cmp(&x[0]))
                .then_with(|| y[1].total_cmp(&x[1]))
                .then_with(|| a.cmp(&b))
        });
        (order, ranks.len())
    }
    #[test]
    #[ignore = "manual candidate-order experiment on exact captured inputs"]
    fn captured_candidate_order_work_probe() {
        // Explicit diagnostic subsets are confined to this ignored experiment.
        // Production ordering and the fixed regression inputs never read this.
        let subset = std::env::var("RRRAH_ORDER_PROBE_INDICES").ok().map(|value| {
            let indices = value
                .split(',')
                .map(|v| v.parse::<usize>().expect("invalid row index"))
                .collect::<Vec<_>>();
            assert!(!indices.is_empty());
            let mut unique = indices.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), indices.len(), "duplicate row index");
            assert!(indices.iter().all(|i| *i < 328));
            indices
        });
        type Row = (usize, usize, usize, [[u64; 2]; 3], [[u64; 3]; 6]);
        let (dense, _): (&[Row], usize) = include!("../../../../tests/fixtures/geometry/dense-cell-176.rs");
        let (coverage82, _): (&[Row], usize) =
            include!("../../../../tests/fixtures/geometry/coverage-cell-82.rs");
        let (coverage103, _): (&[Row], usize) =
            include!("../../../../tests/fixtures/geometry/coverage-cell-103.rs");
        let (fifty, _): (&[Row], u64) = include!("../../../../tests/fixtures/geometry/dense-lineage.rs");
        let (seventy_five, _): (&[Row], u64) =
            include!("../../../../tests/fixtures/geometry/dense-lineage-138.rs");
        for (label, rows) in [
            ("dense1236", dense),
            ("coverage116", coverage82),
            ("coverage328", coverage103),
            ("dense50", fifty),
            ("dense75", seventy_five),
        ] {
            if subset.is_some() && label != "coverage328" {
                continue;
            }
            let (mut order, groups) = vertex_max_order(rows);
            if let Some(indices) = &subset {
                order = indices.clone();
                eprintln!("ORDER_PROBE selected_indices={indices:?}");
            }
            let admit = |_: usize| Some(Box::new(()) as Credit);
            for shared in [false, true] {
                let mut control = None;
                'variant: for alternative in [false, true] {
                    let mut pixel = mode_pixel(&admit, 4096, shared);
                    let mut work = 100_000_000;
                    let mut baseline = subset
                        .clone()
                        .unwrap_or_else(|| (0..rows.len()).collect::<Vec<_>>());
                    baseline.sort_unstable();
                    let mut previous_area = 0.;
                    let mut first_loss = None;
                    for (step, &index) in (if alternative { &order } else { &baseline }).iter().enumerate() {
                        let (patch_index, sample_index, component_count, points, fields) = rows[index];
                        let f = fields.map(|v| v.map(f64::from_bits));
                        let owner = super::super::PixelSourceOwner {
                            patch_index,
                            sample_index,
                            component_count,
                            u: f[0],
                            v: f[1],
                            components: [f[2], f[3], f[4], f[5]],
                        };
                        let points =
                            points.map(|p| kurbo::Point::new(f64::from_bits(p[0]), f64::from_bits(p[1])));
                        if let Err(error) = pixel.paint_triangle(points, owner, &mut work, &|| false) {
                            eprintln!(
                                "ORDER_PROBE label={label} shared={shared} alternative={alternative} refused_at={step} row={index} error={error:?} work={}",
                                100_000_000 - work
                            );
                            assert!(alternative, "captured baseline must complete");
                            continue 'variant;
                        }
                        let area: f64 = pixel
                            .regions
                            .values
                            .iter()
                            .filter_map(|f| f.polygon.moments().map(|m| m.0))
                            .sum();
                        if area < previous_area - 1e-10 && first_loss.is_none() {
                            first_loss = Some((step, index, previous_area, area));
                        }
                        previous_area = area;
                    }
                    let mut integral = [0.; 5];
                    pixel
                        .visit(&mut work, &|| false, &mut |area, centroid, _, owner| {
                            integral[0] += area;
                            for k in 0..owner.component_count {
                                let f = owner.components[k];
                                integral[k + 1] +=
                                    area * f[1].mul_add(centroid.x, f[2].mul_add(centroid.y, f[0]));
                            }
                            Ok(())
                        })
                        .unwrap();
                    let error = control.map_or(0., |old: [f64; 5]| {
                        old.into_iter()
                            .zip(integral)
                            .map(|(a, b)| (a - b).abs())
                            .fold(0., f64::max)
                    });
                    if !alternative {
                        control = Some(integral);
                    }
                    eprintln!(
                        "ORDER_PROBE label={label} shared={shared} alternative={alternative} groups={} work={} fragments={} integral={integral:?} max_error={error}",
                        groups,
                        100_000_000 - work,
                        pixel.fragments()
                    );
                    eprintln!("ORDER_PROBE first_loss={first_loss:?}");
                    if error >= 1e-10 {
                        eprintln!(
                            "ORDER_PROBE REJECTED: changed coverage/color integral; order_indices={order:?}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn rounded_support_overlays_match_exact_unions() {
        type Row = (usize, usize, usize, [[u64; 2]; 3], [[u64; 3]; 6]);
        let (rows, _): (&[Row], usize) = include!("../../../../tests/fixtures/geometry/coverage-cell-103.rs");
        // Reduced ordering counterexamples. Independent Fraction unions are
        // recorded in candidate-order-minimal-union.json and
        // separation-residual-minimal-union.json.
        const CASES: &[(&[usize], u64)] = &[
            (&[174, 39], 4567955829945591764),
            (&[139, 39], 4570225614605276352),
            (&[139, 174], 4563098503674749420),
            (&[139, 174, 39], 4570225614605276352),
            (&[182, 131, 144], 4568435167118576023),
            (&[166, 131, 144], 4568435167118576023),
            (&[166, 182, 144], 4564187911499250066),
            (&[166, 182, 131], 4565560559701183370),
            (&[166, 182, 131, 144], 4568435167118576023),
        ];
        let admit = |_: usize| Some(Box::new(()) as Credit);
        let mut matches = true;
        for shared in [false, true] {
            for &(indices, expected_bits) in CASES {
                let mut orders = vec![vec![]];
                for &index in indices {
                    let mut next = Vec::new();
                    for order in orders {
                        for at in 0..=order.len() {
                            let mut variant = order.clone();
                            variant.insert(at, index);
                            next.push(variant);
                        }
                    }
                    orders = next;
                }
                for order in orders {
                    let mut pixel = mode_pixel(&admit, 4096, shared);
                    let mut work = 100_000_000;
                    for &index in &order {
                        let (patch_index, sample_index, component_count, points, fields) = rows[index];
                        let f = fields.map(|v| v.map(f64::from_bits));
                        let owner = super::super::PixelSourceOwner {
                            patch_index,
                            sample_index,
                            component_count,
                            u: f[0],
                            v: f[1],
                            components: [f[2], f[3], f[4], f[5]],
                        };
                        let points =
                            points.map(|p| kurbo::Point::new(f64::from_bits(p[0]), f64::from_bits(p[1])));
                        pixel.paint_triangle(points, owner, &mut work, &|| false).unwrap();
                    }
                    let mut area = 0.;
                    pixel
                        .visit(&mut work, &|| false, &mut |a, _, _, _| {
                            area += a;
                            Ok(())
                        })
                        .unwrap();
                    let expected = f64::from_bits(expected_bits);
                    eprintln!("ROUNDED_SUPPORT shared={shared} indices={order:?} area={area} exact={expected}");
                    if order.len() == 3 {
                        assert!(
                            (area - expected).abs() < 1e-13,
                            "single-deletion control must pass"
                        );
                    }
                    matches &= (area - expected).abs() < 1e-13;
                }
            }
        }
        assert!(
            matches,
            "rounded support overlays must preserve independently proved input unions"
        );
    }
    #[test]
    fn rounded_support_repairs_roll_back_every_admission_cancel_and_short_quota() {
        let (rows, _): (&[CapturedOrderRow], usize) =
            include!("../../../../tests/fixtures/geometry/coverage-cell-103.rs");
        let decode = |index: usize| {
            let (patch_index, sample_index, component_count, points, fields) = rows[index];
            let f = fields.map(|v| v.map(f64::from_bits));
            (
                points.map(|p| kurbo::Point::new(f64::from_bits(p[0]), f64::from_bits(p[1]))),
                super::super::PixelSourceOwner {
                    patch_index,
                    sample_index,
                    component_count,
                    u: f[0],
                    v: f[1],
                    components: [f[2], f[3], f[4], f[5]],
                },
            )
        };
        for indices in [&[139, 174, 39][..], &[166, 182, 131, 144][..]] {
            let reference_admit = |_: usize| Some(Box::new(()) as Credit);
            let mut reference = mode_pixel(&reference_admit, 4096, false);
            for &index in indices {
                let (points, owner) = decode(index);
                reference
                    .paint_triangle(points, owner, &mut 100_000, &|| false)
                    .unwrap();
            }
            let expected = reference.regions.values.iter().copied().collect::<Vec<_>>();
            let expected_keys = reference.lines.keys.clone();
            for shared in [false, true] {
                let exercise = |refuse: usize, cancel: usize, quota: usize| {
                    let ledger = Ledger::new();
                    let admit = |n| ledger.reserve(n);
                    let mut pixel = mode_pixel(&admit, 4096, shared);
                    for &index in &indices[..indices.len() - 1] {
                        let (points, owner) = decode(index);
                        pixel
                            .paint_triangle(points, owner, &mut 100_000, &|| false)
                            .unwrap();
                    }
                    let before = pixel.regions.values.iter().copied().collect::<Vec<_>>();
                    let keys = pixel.lines.keys.clone();
                    let next = pixel.lines.next;
                    let base = ledger.used();
                    ledger.calls.set(0);
                    ledger.reject.set(refuse);
                    let checks = Cell::new(0);
                    let mut work = quota;
                    let (points, owner) = decode(*indices.last().unwrap());
                    let result = pixel.paint_triangle(points, owner, &mut work, &|| {
                        let at = checks.get();
                        checks.set(at + 1);
                        at == cancel
                    });
                    let counts = (ledger.calls.get(), checks.get(), quota - work);
                    if refuse != usize::MAX || cancel != usize::MAX || quota < 100_000 {
                        let error = if refuse != usize::MAX {
                            PixelPartitionError::Admission
                        } else if cancel != usize::MAX {
                            PixelPartitionError::Cancelled
                        } else {
                            PixelPartitionError::Limit
                        };
                        assert_eq!(result, Err(error));
                        assert_eq!(ledger.used(), base);
                        assert!(pixel.lines.keys == keys);
                        assert_eq!(pixel.lines.next, next);
                        assert_eq!(pixel.lines.map.len(), keys.len());
                        for (i, key) in keys.iter().enumerate() {
                            assert_eq!(pixel.lines.map.get(key), Some(&((i + 1) as u32)));
                        }
                        assert_eq!(pixel.fragments(), before.len());
                        for (a, b) in pixel.regions.values.iter().zip(&before) {
                            assert_fragment(a, b);
                        }
                        assert_summaries(&pixel);
                        ledger.reject.set(usize::MAX);
                        pixel
                            .paint_triangle(points, owner, &mut 100_000, &|| false)
                            .unwrap();
                    } else {
                        result.unwrap();
                    }
                    assert!(pixel.lines.keys == expected_keys);
                    assert_eq!(pixel.fragments(), expected.len());
                    for (a, b) in pixel.regions.values.iter().zip(&expected) {
                        assert_fragment(a, b);
                    }
                    assert_summaries(&pixel);
                    drop(pixel);
                    assert_eq!(ledger.used(), 0);
                    counts
                };
                let (admissions, checks, work) = exercise(usize::MAX, usize::MAX, 100_000);
                for at in 0..admissions {
                    exercise(at, usize::MAX, 100_000);
                }
                for at in 0..checks {
                    exercise(usize::MAX, at, 100_000);
                }
                for quota in 0..work {
                    exercise(usize::MAX, usize::MAX, quota);
                }
                eprintln!(
                    "ROUNDED_SUPPORT_ROLLBACK indices={indices:?} shared={shared} admissions={admissions} cancellations={checks} short_quotas={work}; exact geometry/fields/tags/line IDs and charge restored, retry exact, final=0"
                );
            }
        }
    }
    #[test]
    fn captured_reordered_coverage_matches_exact_union() {
        let (rows, _): (&[CapturedOrderRow], usize) =
            include!("../../../../tests/fixtures/geometry/coverage-cell-103.rs");
        let (order, _) = vertex_max_order(rows);
        let admit = |_: usize| Some(Box::new(()) as Credit);
        for shared in [false, true] {
            let mut pixel = mode_pixel(&admit, 4096, shared);
            let mut work = 100_000_000;
            let mut previous = 0.;
            for (step, &index) in order.iter().enumerate() {
                let (patch_index, sample_index, component_count, points, fields) = rows[index];
                let f = fields.map(|v| v.map(f64::from_bits));
                let owner = super::super::PixelSourceOwner {
                    patch_index,
                    sample_index,
                    component_count,
                    u: f[0],
                    v: f[1],
                    components: [f[2], f[3], f[4], f[5]],
                };
                let points = points.map(|p| kurbo::Point::new(f64::from_bits(p[0]), f64::from_bits(p[1])));
                pixel.paint_triangle(points, owner, &mut work, &|| false).unwrap();
                let mut area = 0.;
                pixel
                    .visit(&mut work, &|| false, &mut |a, _, _, _| {
                        area += a;
                        Ok(())
                    })
                    .unwrap();
                assert!(
                    area <= 1. + 1e-10 && area >= previous - 1e-10,
                    "shared={shared} index={index} area={area} previous={previous}"
                );
                // Exact independent controls around the original first loss.
                let exact_prefix = match step {
                    75 => Some(4605982357540248363),
                    76 => Some(4605989400735879313),
                    _ => None,
                };
                if let Some(bits) = exact_prefix {
                    assert!(
                        (area - f64::from_bits(bits)).abs() < 1e-13,
                        "shared={shared} prefix={} area={area}",
                        step + 1
                    );
                }
                previous = area;
            }
            let expected = f64::from_bits(4607182411973925879);
            assert!(
                (previous - expected).abs() < 1e-13,
                "shared={shared} area={previous} exact={expected}"
            );
        }
    }
    #[test]
    fn captured_dense_cell_matches_flat_every_prefix() {
        type Row = (usize, usize, usize, [[u64; 2]; 3], [[u64; 3]; 6]);
        let (rows, page_remaining): (&[Row], usize) = include!("../../../../tests/fixtures/geometry/dense-cell-176.rs");
        assert_eq!(rows.len(), 1236);
        let admit = |_: usize| Some(Box::new(()) as Credit);
        let mut flat = mode_pixel(&admit, 4096, false);
        let mut shared = mode_pixel(&admit, 4096, true);
        let mut flat_work = 100_000_000;
        let mut shared_work = flat_work;
        #[cfg(feature = "experimental-shared-partitions")]
        let mut reference = mode_pixel(&admit,4096,false);
        #[cfg(feature = "experimental-shared-partitions")]
        let mut reference_work = flat_work;
        let mut first_page_quota_crossing = [None, None];
        for (index, &(patch_index, sample_index, component_count, points, fields)) in rows.iter().enumerate() {
            let f = fields.map(|v| v.map(f64::from_bits));
            let owner = super::super::PixelSourceOwner {
                patch_index, sample_index, component_count,
                u: f[0], v: f[1], components: [f[2], f[3], f[4], f[5]],
            };
            let points = points.map(|p| kurbo::Point::new(f64::from_bits(p[0]), f64::from_bits(p[1])));
            flat.paint_triangle(points, owner, &mut flat_work, &|| false).unwrap();
            shared.paint_triangle(points, owner, &mut shared_work, &|| false).unwrap();
            assert_eq!(flat.fragments(), shared.fragments(), "paint {index}");
            assert!(flat.lines.keys == shared.lines.keys, "paint {index}");
            assert_eq!(flat.lines.next, shared.lines.next);
            for (a, b) in flat.regions.values.iter().zip(shared.regions.values.iter()) {
                assert_fragment(a, b);
            }
            assert_summaries(&shared);
            #[cfg(feature = "experimental-shared-partitions")]
            {
                reference.paint_triangle_impl::<false>(points,owner,&mut reference_work,&||false).unwrap();
                assert_eq!(reference.fragments(),flat.fragments(),"uncached paint {index}");
                assert!(reference.lines.keys==flat.lines.keys && reference.lines.map==flat.lines.map);
                assert_eq!(reference.lines.next,flat.lines.next);
                for (a,b) in reference.regions.values.iter().zip(flat.regions.values.iter()) {assert_fragment(a,b);}
            }
            for (slot, work) in first_page_quota_crossing.iter_mut().zip([flat_work, shared_work]) {
                if slot.is_none() && 100_000_000 - work > page_remaining { *slot = Some(index); }
            }
        }
        eprintln!("captured dense1236 flat_work={} shared_work={} page_remaining={page_remaining} first_page_quota_crossing={first_page_quota_crossing:?}; exact prefixes/fields/tags/line IDs", 100_000_000-flat_work, 100_000_000-shared_work);
    }
    #[test]
    fn shared_chunk_guard_does_not_carry_unknown_merge_or_zero_area_records() {
        let admit = |_: usize| Some(Box::new(()) as Credit);
        let rectangle = |a: f64, b: f64| {
            let mut f = fragment(10);
            f.polygon = super::super::Polygon::empty();
            for p in [
                kurbo::Point::new(a, 0.),
                kurbo::Point::new(b, 0.),
                kurbo::Point::new(b, 1.),
                kurbo::Point::new(a, 1.),
            ] {
                f.polygon.push(p).unwrap();
            }
            f.no_merge_with_previous = false;
            Fragment::new(f.polygon,f.owner,f.has_area,f.no_merge_with_previous)
        };
        for include_zero in [false, true] {
            let mut flat = mode_pixel(&admit, 128, false);
            let mut shared = mode_pixel(&admit, 128, true);
            for pixel in [&mut flat, &mut shared] {
                let mut first = rectangle(0., 0.25);
                first.no_merge_with_previous = include_zero;
                pixel.regions.sink_push(first).unwrap();
                if include_zero {
                    let mut zero = fragment(10);
                    zero.polygon = super::super::Polygon::empty();
                    zero.has_area = false;
                    zero = Fragment::new(zero.polygon,zero.owner,false,zero.no_merge_with_previous);
                    pixel.regions.sink_push(zero).unwrap();
                }
                let mut second = rectangle(0.25, 0.5);
                second.no_merge_with_previous = include_zero;
                pixel.regions.sink_push(second).unwrap();
            }
            let RegionValues::Shared(values) = &shared.regions.values else {
                panic!()
            };
            assert!(!values.segments.values[0].chunk.summary.eligible);
            let points = [
                kurbo::Point::new(0.75, 0.75),
                kurbo::Point::new(0.875, 0.75),
                kurbo::Point::new(0.75, 0.875),
            ];
            flat.paint_triangle(points, fragment(2000).owner, &mut 1_000_000, &|| false)
                .unwrap();
            shared
                .paint_triangle(points, fragment(2000).owner, &mut 1_000_000, &|| false)
                .unwrap();
            assert_eq!(flat.fragments(), 2);
            assert_eq!(shared.fragments(), 2);
            for (a, b) in flat.regions.values.iter().zip(shared.regions.values.iter()) {
                assert_fragment(a, b);
            }
            assert!(flat.lines.keys == shared.lines.keys);
            assert_summaries(&shared);
            let mut area = 0.;
            shared
                .visit(&mut 10000, &|| false, &mut |a, _, _, _| {
                    area += a;
                    Ok(())
                })
                .unwrap();
            assert_eq!(area, 0.5 + 1. / 128.);
        }
    }
    fn fill_line_capacity(pixel: &mut OrderedPixel<'_>) {
        while pixel.lines.map.len() < pixel.lines.map.capacity() {
            let key = LineKey::Axis(0, (10. + pixel.lines.map.len() as f64).to_bits());
            let mut transaction = pixel.lines.transaction();
            transaction.intern(key, &mut 10000, &|| false).unwrap();
            transaction.commit();
        }
    }
    #[test]
    fn actual_shared_kernel_every_mutation_failure_rolls_back_payload_and_lines() {
        exercise_shared_kernel_faults(false);
    }
    #[test]
    fn actual_shared_kernel_joint_line_growth_and_payload_failures_rollback() {
        exercise_shared_kernel_faults(true);
    }
    fn exercise_shared_kernel_faults(force_growth: bool) {
        let reference_admit = |_: usize| Some(Box::new(()) as Credit);
        let mut reference = mode_pixel(&reference_admit, 512, false);
        populate(&mut reference);
        if force_growth {
            fill_line_capacity(&mut reference);
        }
        cut_middle(&mut reference);
        let expected = reference.regions.values.iter().copied().collect::<Vec<_>>();
        let expected_keys = reference.lines.keys.clone();
        let exercise = |refuse: usize, cancel: usize, quota: usize| {
            let ledger = Ledger::new();
            let admit = |n| ledger.reserve(n);
            let mut pixel = mode_pixel(&admit, 512, true);
            populate(&mut pixel);
            if force_growth {
                fill_line_capacity(&mut pixel);
            }
            let before = pixel.regions.values.iter().copied().collect::<Vec<_>>();
            let keys = pixel.lines.keys.clone();
            let next_id = pixel.lines.next;
            let base = ledger.used();
            ledger.calls.set(0);
            ledger.reject.set(refuse);
            let checks = Cell::new(0);
            let mut work = quota;
            let x = (42 % 16) as f64 / 16.;
            let y = (42 / 16) as f64 / 16.;
            let d = 1. / 256.;
            let points = [
                kurbo::Point::new(x + d / 4., y + d / 4.),
                kurbo::Point::new(x + 3. * d / 8., y + d / 4.),
                kurbo::Point::new(x + d / 4., y + 3. * d / 8.),
            ];
            let result = pixel.paint_triangle(points, fragment(2000).owner, &mut work, &|| {
                let at = checks.get();
                checks.set(at + 1);
                at == cancel
            });
            let counts = (ledger.calls.get(), checks.get(), quota - work);
            if refuse != usize::MAX || cancel != usize::MAX || quota < 100_000 {
                let expected_error = if refuse != usize::MAX {
                    PixelPartitionError::Admission
                } else if cancel != usize::MAX {
                    PixelPartitionError::Cancelled
                } else {
                    PixelPartitionError::Limit
                };
                assert_eq!(result, Err(expected_error));
                assert_summaries(&pixel);
                assert_eq!(ledger.used(), base);
                assert!(pixel.lines.keys == keys);
                assert_eq!(pixel.lines.next, next_id);
                assert_eq!(pixel.lines.map.len(), keys.len());
                for (i, key) in keys.iter().enumerate() {
                    assert_eq!(pixel.lines.map.get(key), Some(&((i + 1) as u32)));
                }
                assert_eq!(pixel.fragments(), before.len());
                for (a, b) in pixel.regions.values.iter().zip(&before) {
                    assert_fragment(a, b);
                }
                ledger.reject.set(usize::MAX);
                pixel
                    .paint_triangle(points, fragment(2000).owner, &mut 100_000, &|| false)
                    .unwrap();
            } else {
                result.unwrap();
            }
            assert_summaries(&pixel);
            assert_eq!(pixel.fragments(), expected.len());
            assert!(pixel.lines.keys == expected_keys);
            for (a, b) in pixel.regions.values.iter().zip(&expected) {
                assert_fragment(a, b);
            }
            let mut area = 0.;
            pixel
                .visit(&mut 1_000_000, &|| false, &mut |a, _, _, _| {
                    area += a;
                    Ok(())
                })
                .unwrap();
            assert_eq!(area, 1. / 1024.);
            drop(pixel);
            assert_eq!(ledger.used(), 0);
            counts
        };
        let (admissions, checks, work) = exercise(usize::MAX, usize::MAX, 100_000);
        for at in 0..admissions {
            exercise(at, usize::MAX, 100_000);
        }
        for at in 0..checks {
            exercise(usize::MAX, at, 100_000);
        }
        for quota in 0..work {
            exercise(usize::MAX, usize::MAX, quota);
        }
        eprintln!(
            "actual shared kernel force_line_growth={force_growth} rollback admissions={admissions} cancellations={checks} short_quotas={work}; exact payload/line IDs/area; final=0"
        );
    }
    #[test]
    fn repeated_tail_replacements_keep_charged_storage_constant() {
        let ledger = Ledger::new();
        let admit = |n| ledger.reserve(n);
        let input = (0..128).map(fragment).collect::<Vec<_>>();
        let source = SharedFragments::from_values(&input, &admit, 256, &mut 10000, &|| false).unwrap();
        let mut current = source
            .with_tail(128, &[fragment(1000)], &mut 10000, &|| false)
            .unwrap();
        let stable = ledger.used();
        for index in 1001..1513 {
            current = current
                .with_tail(128, &[fragment(index)], &mut 10000, &|| false)
                .unwrap();
            assert_eq!(ledger.used(), stable, "retained payload accumulated");
            assert_fragment(current.get(128).unwrap(), &fragment(index));
            for i in 0..128 {
                assert!(std::ptr::eq(current.get(i).unwrap(), source.get(i).unwrap()));
            }
        }
        drop(current);
        drop(source);
        assert_eq!(ledger.used(), 0);
    }
}
