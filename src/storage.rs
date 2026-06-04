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
