use binrw::BinRead;
use binrw::BinWrite;
use binrw::binrw;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;

const BLOCK_SIZE: u16 = 1024;
const SUPERBLOCK_SIZE: usize = std::mem::size_of::<SuperBlock>();
const INODE_SIZE: usize = std::mem::size_of::<INode>();
// const INODE_PER_BLOCK: usize = BLOCK_SIZE as usize / INODE_SIZE;
const INODE_AREA_SIZE: usize = BLOCK_SIZE as usize;

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

    pub fn instantiate_disk(&mut self) -> Result<(), FSError> {
        self.disk
            .write(&[0; std::mem::size_of::<FileSystem>()])
            .unwrap();
        self.disk.seek(std::io::SeekFrom::Start(0)).unwrap();
        match self.fs.write_be(&mut self.disk) {
            Ok(()) => {}
            Err(e) => return Err(FSError::BinRw(e)),
        };
        // root inode = 1
        let _inode_number = self.make_directory(1)?;
        Ok(())
    }

    /// Opens a file in one of various different ways 
    fn create_file(){
        // locate directory to put file in
        // get next free i node
        // get next free i node storage position
        // get next free block location
        // point inode block 1 to that new location
        // write i node to its location
        // create entry in directory
    }

    fn write(){
        // locate file in directory
        // read inode
        // see if the data you are writing to will need an extra block
        // if it needs extra blocks
        ////find extra blocks
        ////assign new blocks in inode
        ////split data and write to those blocks
        ////enforce a limit of 7KB
        //write inode changes
    }

    fn stat(){
        // locate file's inode
        // just print out the inode
    }

    fn rename() {
        // locate files inode entry in directory
        // change the entry name (the write will need to happen in the block that the directory
        // entry points to
    }

    fn rmdir() {
        // must be recursive
        // delete all files by inode
        // call rmdir on each directory
        // change next free inode if needed
    }

    fn delete_file() {
        // locate inode
        // mark blocks that inode points to as free
        // mark inode as free
        // change next free inode if needed
    }
    fn read() {
        // locate files inode
        // read n bytes into vector and return that vector
    }

    fn make_directory(&mut self, prev_inode: u16) -> Result<u16, FSError> {
        let inode_number = self.fs.super_block.next_free_inode_pos;
        self.fs.super_block.next_free_inode_pos += 1;
        let mut inode = INode::new(EntryType::Directory);
        inode.block_1 = match self.get_free_block() {
            Ok(b) => b,
            Err(e) => return Err(FSError::Simple(format!("make_directory: {}", e))),
        };
        // create the . and .. entries
        let dot = DirectoryEntry::new(inode_number, ".");
        let dotdot = DirectoryEntry::new(prev_inode, "..");

        let block_pos = self.find_block_offset(&inode.block_1);
        let _ = self.disk.seek(std::io::SeekFrom::Start(block_pos));

        if let Err(e) = dot.write_be(&mut self.disk) {
            return Err(FSError::BinRw(e));
        }
        if let Err(e) = dotdot.write_be(&mut self.disk) {
            return Err(FSError::BinRw(e));
        }
        inode.file_size += (2 * std::mem::size_of::<DirectoryEntry>()) as u16;
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

    fn find_block_offset(&self, block_id: &u16) -> u64 {
        SUPERBLOCK_SIZE as u64
            + BLOCK_SIZE as u64
            + INODE_AREA_SIZE as u64
            + BLOCK_SIZE as u64
            + *block_id as u64 * BLOCK_SIZE as u64
    }

    fn find_inode_offset(&self, idx: &u16) -> u64 {
        SUPERBLOCK_SIZE as u64 + BLOCK_SIZE as u64 + *idx as u64 * INODE_SIZE as u64
    }

    fn get_free_block(&mut self) -> Result<u16, String> {
        let idx = self.fs.super_block.next_free_block;
        if idx >= 65535 {
            return Err("get_free_block: No more blocks to allocate".to_string());
        }
        self.mark_block_as_used(&idx);
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
    next_free_inode_pos: u16,
    next_free_block: u16,
    padding: [u8; 1004],
}

impl SuperBlock {
    pub fn new(disk_size: u32, root_inode_pos: u16) -> Self {
        SuperBlock {
            magic_number: 0xDEADBEEF,
            n_inode: 1,
            n_inode_bitmap_blocks: 1,
            n_blocks: (disk_size / BLOCK_SIZE as u32) as u16,
            max_file_size: 7 * BLOCK_SIZE,
            block_size: BLOCK_SIZE,
            root_inode_pos,
            next_free_inode_pos: 1,
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
pub struct DirectoryEntry {
    inode_number: u16,
    file_name: [u8; 30],
}

impl DirectoryEntry {
    fn new(inode_number: u16, fname: &str) -> Self {
        let mut t = [0u8; 30];
        t[..fname.len()].copy_from_slice(fname.as_bytes());
        DirectoryEntry {
            inode_number,
            file_name: t,
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
    inode_area: [u8; INODE_AREA_SIZE],
}

impl FileSystem {
    pub fn new() -> Self {
        let root_inode_pos = std::mem::size_of::<FileSystem>() + BLOCK_SIZE as usize;
        FileSystem {
            super_block: SuperBlock::new(512u32 * BLOCK_SIZE as u32, root_inode_pos as u16),
            block_bitmap: [0; BLOCK_SIZE as usize],
            inode_area: [0; INODE_AREA_SIZE],
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

        let des_root_inode_pos = std::mem::size_of::<FileSystem>() + BLOCK_SIZE as usize;
        if controller.fs.super_block.root_inode_pos != des_root_inode_pos as u16 {
            errors.push(format!(
                "root_inode_pos: want={}, got={}",
                des_root_inode_pos, controller.fs.super_block.root_inode_pos
            ));
        }

        if controller.fs.super_block.next_free_inode_pos != 2 {
            errors.push(format!(
                "next_free_inode_pos: want={}, got={}",
                2, controller.fs.super_block.next_free_inode_pos
            ));
        }

        // TODO: test inode position
        // TODO: test inodes block's content


        let _ = remove_test_disk();
        assert!(errors.is_empty(), "\n{}", errors.join("\n"));
    }
}
