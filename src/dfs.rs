use binrw::BinRead;
use binrw::BinWrite;
use binrw::binrw;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;

const BLOCK_SIZE: u16 = 1024;
const SUPERBLOCK_SIZE: usize = std::mem::size_of::<SuperBlock>();
const INODE_SIZE: usize = std::mem::size_of::<INode>();

#[derive(Debug)]
pub enum FSError {
    BinRw(binrw::Error),
    Simple(String),
}

impl std::fmt::Display for FSError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            FSError::BinRw(e) => write!(f, "binrw error: {e}"),
            FSError::Simple(e) => write!(f, "simple error: {e}"),
        }
    }
}

impl From<binrw::Error> for FSError {
    fn from(value: binrw::Error) -> Self {
        FSError::BinRw(value)
    }
}

pub struct FSController {
    fs: FileSystem,
    disk: File,
}

impl FSController {
    pub fn new(fname: &str) -> Self {
        let file = match OpenOptions::new().write(true).open(fname) {
            Ok(file) => file,
            Err(why) => panic!("couldnt open {}: {}", fname, why),
        };
        FSController {
            fs: FileSystem::new(),
            disk: file,
        }
    }

    #[allow(dead_code)]
    pub fn open(fname: &str) -> FSController {
        let mut file = match OpenOptions::new().write(true).read(true).open(fname) {
            Ok(file) => file,
            Err(why) => panic!("couldnt open {}: {}", fname, why),
        };
        let file_system = FileSystem::read(&mut file).unwrap();
        FSController {
            fs: file_system,
            disk: file,
        }
    }

    pub fn instantiate_disk(&mut self) -> Result<u16, FSError> {
        self.disk
            .write(&[0; std::mem::size_of::<FileSystem>()])
            .unwrap();
        self.disk.seek(std::io::SeekFrom::Start(0)).unwrap();
        if let Err(e) = self.fs.write_be(&mut self.disk) {
            return Err(FSError::BinRw(e));
        };
        self.make_directory()
    }

    fn make_directory(&mut self) -> Result<u16, FSError> {
        let inode_number = self.fs.super_block.next_free_inode_idx;
        self.fs.super_block.next_free_inode_idx += 1;
        let mut inode = INode::new(EntryType::Directory);
        inode.block_1 = match self.get_free_block() {
            Ok(b) => b,
            Err(e) => return Err(FSError::Simple(format!("make_directory: {}", e))),
        };
        self.mark_block_as_used(&inode.block_1);
        let _ = self.disk.seek(std::io::SeekFrom::Start(
            self.find_inode_offset(&inode_number),
        ));
        inode.write_be(&mut self.disk)?;
        Ok(inode_number)
    }

    fn mark_block_as_used(&mut self, block_num: &u16) {
        let byte_idx = block_num / 8;
        let remainder = block_num % 8;
        let bit_mask = 7 - remainder;
        self.fs.block_bitmap[byte_idx as usize] |= bit_mask as u8;
    }

    fn find_inode_offset(&self, number: &u16) -> u64 {
        SUPERBLOCK_SIZE as u64 + BLOCK_SIZE as u64 + *number as u64 * INODE_SIZE as u64
    }

    fn get_free_block(&mut self) -> Result<u16, String> {
        let idx = self.fs.super_block.next_free_block;
        if idx == 65535 {
            return Err("get_free_block: No more blocks to allocate".to_string());
        }
        // TODO: need some way of updating and then writing to disk the consumed inode in the inode block
        self.fs.super_block.next_free_block += 1;
        Ok(idx)
    }

    pub fn sync(&mut self) -> Result<(), FSError> {
        self.disk
            .write(&[0; std::mem::size_of::<FileSystem>()])
            .unwrap();
        self.disk.seek(std::io::SeekFrom::Start(0)).unwrap();
        if let Err(e) = self.fs.write_be(&mut self.disk) {
            return Err(FSError::BinRw(e));
        };
        let _ = self.disk.seek(std::io::SeekFrom::Start(0_u64));
        self.fs.super_block.write_be(&mut self.disk)?;
        Ok(())
    }
}

#[repr(C)]
#[binrw]
#[derive(Debug)]
#[brw(big)]
pub struct SuperBlock {
    pub magic_number: u32,
    n_inode: u16,
    n_inode_bitmap_blocks: u16,
    n_blocks: u16,
    max_file_size: u16,
    block_size: u16,
    root_inode_pos: u16,
    next_free_inode_idx: u16,
    next_free_block: u16,
    padding: [u8; 1004],
}

impl SuperBlock {
    pub fn new(disk_size: u32) -> Self {
        SuperBlock {
            magic_number: 0xDEADBEEF,
            n_inode: 1,
            n_inode_bitmap_blocks: 1,
            n_blocks: (disk_size / BLOCK_SIZE as u32) as u16,
            max_file_size: 7 * BLOCK_SIZE,
            block_size: BLOCK_SIZE,
            // TODO: this should point to where the root inode is, not an index
            root_inode_pos: 1,
            next_free_inode_idx: 1,
            next_free_block: 1,
            padding: [1; 1004],
        }
    }
}

#[repr(C)]
#[binrw]
#[brw(big)]
struct INode {
    mode: u16,
    uid: u16,
    gid: u16,
    file_size: u16,
    access_time: u16,
    modification_time: u16,
    status_change_time: u16,
    block_1: u16,
    block_2: u16,
    block_3: u16,
    block_4: u16,
    block_5: u16,
    block_6: u16,
    indirect_block: u16,
    double_indirect_block: u16,
    unused: u16,
}

#[allow(dead_code)]
enum EntryType {
    Directory,
    File,
}

impl INode {
    pub fn new(etype: EntryType) -> Self {
        INode {
            mode: match etype {
                EntryType::File => 0xF000,
                EntryType::Directory => 0xD000,
            },
            uid: 1000,
            gid: 1000,
            file_size: 0,
            access_time: 0,
            modification_time: 0,
            status_change_time: 0,
            block_1: 0,
            block_2: 0,
            block_3: 0,
            block_4: 0,
            block_5: 0,
            block_6: 0,
            indirect_block: 0,
            double_indirect_block: 0,
            unused: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
#[binrw]
#[brw(big)]
pub struct FileSystem {
    pub super_block: SuperBlock,
    block_bitmap: [u8; BLOCK_SIZE as usize],
    inode_block: [u8; 16 * BLOCK_SIZE as usize],
}

impl FileSystem {
    pub fn new() -> Self {
        FileSystem {
            super_block: SuperBlock::new(512u32 * BLOCK_SIZE as u32),
            block_bitmap: [0; BLOCK_SIZE as usize],
            inode_block: [0; 16 * BLOCK_SIZE as usize],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_disk(size: usize) -> std::io::Result<()> {
        let file = File::create("./TESTFS")?;
        file.set_len(size as u64)?;
        Ok(())
    }
    fn remove_test_disk() -> std::io::Result<()> {
        std::fs::remove_file("./TESTFS")?;
        Ok(())
    }

    #[test]
    fn fs_instantiation_confirmation() {
        let mut errors: Vec<String> = Vec::new();
        if let Err(e) = create_test_disk(512 * 1024) {
            errors.push(format!("create_test_disk: {}", e));
        }

        {
            let mut controller = FSController::new("./TESTFS");
            match controller.instantiate_disk() {
                Ok(_) => {}
                Err(e) => errors.push(format!("instantiate_disk: {}", e)),
            }
        }

        let mut controller = FSController::open("./TESTFS");
        match controller.instantiate_disk() {
            Ok(_) => {}
            Err(e) => errors.push(format!("instantiate_disk: {}", e)),
        }

        if controller.fs.super_block.magic_number != 0xDEADBEEF {
            errors.push(format!(
                "magic number read back was not 0xDEADBEEF, got {:X}",
                controller.fs.super_block.magic_number
            ))
        }

        let _ = remove_test_disk();
        assert!(errors.is_empty(), "\n{}", errors.join("\n"));
    }
}
