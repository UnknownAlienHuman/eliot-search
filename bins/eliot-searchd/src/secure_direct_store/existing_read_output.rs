//! Every output attempt borrows the command's original native read operation.

use std::io::{self, Write};

use crate::direct_store::ReadOnlyStore;

pub struct NativeOutput<'a, 'owner, W> {
    child: &'a ReadOnlyStore<'owner>,
    writer: &'a mut W,
}

impl<'a, 'owner, W: Write> NativeOutput<'a, 'owner, W> {
    pub(crate) const fn new(child: &'a ReadOnlyStore<'owner>, writer: &'a mut W) -> Self {
        Self { child, writer }
    }

    fn checkpoint(&self) -> io::Result<()> {
        self.child
            .native_checkpoint()
            .map_err(|_| io::Error::other("SERVICE_NATIVE_READ_REFUSED"))
    }

    fn finish<T>(&self, result: io::Result<T>) -> io::Result<T> {
        if result.is_err() {
            self.child.cancel_output();
        }
        self.checkpoint()?;
        result
    }
}

impl<W: Write> Write for NativeOutput<'_, '_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = self.child.write_output(self.writer, bytes);
        if matches!(result, Ok(0)) && !bytes.is_empty() {
            self.child.cancel_output();
        }
        self.finish(result)
    }

    fn flush(&mut self) -> io::Result<()> {
        let result = self.child.flush_output(self.writer);
        self.finish(result)
    }
}
