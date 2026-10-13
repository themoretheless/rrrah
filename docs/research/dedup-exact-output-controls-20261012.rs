
#[cfg(test)]
mod cancelled_output_tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    struct Writer<'a> { calls: &'a Cell<usize>, bytes: &'a RefCell<Vec<u8>>, mode: u8 }
    impl io::Write for Writer<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls.set(self.calls.get() + 1);
            match self.mode {
                0 if self.calls.get() <= 3 => return Err(io::ErrorKind::Interrupted.into()),
                2 if self.calls.get() % 2 == 1 => return Err(io::ErrorKind::Interrupted.into()),
                3 => return Ok(0),
                4 => return Err(io::ErrorKind::BrokenPipe.into()),
                _ => {}
            }
            let count = if self.mode == 0 { bytes.len() } else { bytes.len().min(2) };
            self.bytes.borrow_mut().extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    fn snapshot() -> (tempfile::TempDir, ContentSnapshot) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source");
        fs::write(&path, b"0123456789").unwrap();
        let snapshot = ContentSnapshot::read(&path, 10, || false).unwrap();
        (root, snapshot)
    }
    #[test]
    fn interrupted_output_cancels_before_next_write_attempt() {
        let (_root, snapshot) = snapshot();
        let calls = Cell::new(0); let bytes = RefCell::new(Vec::new());
        let writer = Writer { calls: &calls, bytes: &bytes, mode: 0 };
        assert!(matches!(snapshot.copy_range(1, 8, writer, || calls.get() >= 3), Err(SnapshotError::Cancelled)));
        assert_eq!(calls.get(), 3);
        assert!(bytes.borrow().is_empty());
    }
    #[test]
    fn partial_output_cancels_without_writing_remaining_bytes() {
        let (_root, snapshot) = snapshot();
        let calls = Cell::new(0); let bytes = RefCell::new(Vec::new());
        let writer = Writer { calls: &calls, bytes: &bytes, mode: 1 };
        assert!(matches!(snapshot.copy_range(1, 8, writer, || calls.get() >= 2), Err(SnapshotError::Cancelled)));
        assert_eq!(calls.get(), 2);
        assert_eq!(&*bytes.borrow(), b"1234");
    }
    #[test]
    fn healthy_short_interrupted_output_preserves_exact_range() {
        let (_root, snapshot) = snapshot();
        let calls = Cell::new(0); let bytes = RefCell::new(Vec::new());
        let writer = Writer { calls: &calls, bytes: &bytes, mode: 2 };
        snapshot.copy_range(1, 8, writer, || false).unwrap();
        assert_eq!(&*bytes.borrow(), b"12345678");
        assert_eq!(calls.get(), 8);
    }
    #[test]
    fn zero_progress_and_output_errors_remain_typed_io_failures() {
        let (_root, snapshot) = snapshot();
        for (mode, expected) in [(3, io::ErrorKind::WriteZero), (4, io::ErrorKind::BrokenPipe)] {
            let calls = Cell::new(0); let bytes = RefCell::new(Vec::new());
            let writer = Writer { calls: &calls, bytes: &bytes, mode };
            assert!(matches!(snapshot.copy_range(1, 8, writer, || false), Err(SnapshotError::Io(e)) if e.kind() == expected));
            assert_eq!(calls.get(), 1);
            assert!(bytes.borrow().is_empty());
        }
    }
}
