use std::fs::{OpenOptions, File};
use std::path::Path;
use memmap2::MmapMut;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GraphHeader {
    pub magic: [u8; 8],
    pub version: u32,
    pub node_count: u32,
    pub edge_count: u32,
    pub embedding_dim: u32,
}

pub struct MmappedStorage {
    pub file: File,
    pub mmap: MmapMut,
}

impl MmappedStorage {
    pub fn open<P: AsRef<Path>>(path: P, size: usize) -> anyhow::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        file.set_len(size as u64)?;

        let mmap = unsafe { MmapMut::map_mut(&file)? };

        Ok(Self { file, mmap })
    }

    pub fn as_slice<T: Pod>(&self, offset: usize, count: usize) -> &[T] {
        let start = offset;
        let end = start + count * std::mem::size_of::<T>();
        bytemuck::cast_slice(&self.mmap[start..end])
    }

    pub fn as_slice_mut<T: Pod>(&mut self, offset: usize, count: usize) -> &mut [T] {
        let start = offset;
        let end = start + count * std::mem::size_of::<T>();
        bytemuck::cast_slice_mut(&mut self.mmap[start..end])
    }

    pub fn header_mut(&mut self) -> &mut GraphHeader {
        bytemuck::from_bytes_mut(&mut self.mmap[0..std::mem::size_of::<GraphHeader>()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER_SIZE: usize = std::mem::size_of::<GraphHeader>();

    #[test]
    fn open_sets_file_length() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.bin");
        let storage = MmappedStorage::open(&path, 4096).unwrap();
        assert_eq!(storage.file.metadata().unwrap().len(), 4096);
        assert_eq!(storage.mmap.len(), 4096);
    }

    #[test]
    fn header_roundtrip_and_persistence_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.bin");
        {
            let mut storage = MmappedStorage::open(&path, 4096).unwrap();
            let header = storage.header_mut();
            header.magic = *b"PHYSGRPH";
            header.version = 2;
            header.node_count = 7;
            header.edge_count = 11;
            header.embedding_dim = 768;
            storage.mmap.flush().unwrap();
        }
        let mut storage = MmappedStorage::open(&path, 4096).unwrap();
        let header = storage.header_mut();
        assert_eq!(&header.magic, b"PHYSGRPH");
        assert_eq!(header.version, 2);
        assert_eq!(header.node_count, 7);
        assert_eq!(header.edge_count, 11);
        assert_eq!(header.embedding_dim, 768);
    }

    #[test]
    fn typed_slice_roundtrip_at_offset() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.bin");
        let mut storage = MmappedStorage::open(&path, 4096).unwrap();
        // Write f32s after the header region, read them back through as_slice.
        let written: Vec<f32> = vec![1.5, -2.25, 0.0, 42.0];
        storage.as_slice_mut::<f32>(HEADER_SIZE, 4).copy_from_slice(&written);
        assert_eq!(storage.as_slice::<f32>(HEADER_SIZE, 4), written.as_slice());
    }
}
