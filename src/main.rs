use binrw::BinRead;
use binrw::BinWrite;
use binrw::binrw;
use std::env;
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

struct FSController {}
impl FSController {
    pub fn new() -> Self {
        FSController {}
    }
}

#[repr(C)]
#[binrw]
#[derive(Debug)]
#[brw(big)]
struct SuperBlock {
    magic_number: u32,
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
            max_file_size: 7 * BLOCK_SIZE as u16,
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
struct FileSystem {
    super_block: SuperBlock,
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

    pub fn open(disk_file_path: &String) -> FileSystem {
        let mut file = match File::open(&disk_file_path) {
            Ok(file) => file,
            Err(why) => panic!("couldnt open {}: {}", disk_file_path, why),
        };
        let fs = FileSystem::read(&mut file).unwrap();
        fs
    }

    pub fn instantiate_disk(&mut self, disk_file_path: &String) -> Result<u16, FSError> {
        let mut file = match OpenOptions::new().write(true).open(&disk_file_path) {
            Ok(file) => file,
            Err(why) => panic!("couldnt open {}: {}", disk_file_path, why),
        };

        file.write(&[0; std::mem::size_of::<FileSystem>()]).unwrap();
        file.seek(std::io::SeekFrom::Start(0)).unwrap();
        if let Err(e) = self.write_be(&mut file) {
            return Err(FSError::BinRw(e));
        };
        self.make_directory(&mut file)
    }

    fn mark_block_as_used(&mut self, block_num: &u16) {
        let byte_idx = block_num / 8;
        let remainder = block_num % 8;
        let bit_mask = 7 - remainder;
        self.block_bitmap[byte_idx as usize] |= bit_mask as u8;
    }

    fn find_inode_offset(&self, number: &u16) -> u64 {
        SUPERBLOCK_SIZE as u64 + BLOCK_SIZE as u64 + *number as u64 * INODE_SIZE as u64
    }

    fn make_directory(&mut self, disk: &mut File) -> Result<u16, FSError> {
        let inode_number = self.super_block.next_free_inode_idx;
        self.super_block.next_free_inode_idx += 1;
        let mut inode = INode::new(EntryType::Directory);
        inode.block_1 = match self.get_free_block() {
            Ok(b) => b,
            Err(e) => return Err(FSError::Simple(format!("make_directory: {}", e))),
        };
        self.mark_block_as_used(&inode.block_1);
        let _ = disk.seek(std::io::SeekFrom::Start(
            self.find_inode_offset(&inode_number),
        ));
        inode.write_be(disk)?;
        Ok(inode_number)
    }

    fn get_free_block(&mut self) -> Result<u16, String> {
        let idx = self.super_block.next_free_block;
        if idx == 65535 {
            return Err("get_free_block: No more blocks to allocate".to_string());
        }
        // TODO: need some way of updating and then writing to disk the consumed inode in the inode block
        self.super_block.next_free_block += 1;
        Ok(idx)
    }

    pub fn sync(&mut self, fname: &String) -> Result<(), FSError> {
        let mut file = match OpenOptions::new().write(true).open(&fname) {
            Ok(file) => file,
            Err(why) => panic!("couldnt open {}: {}", fname, why),
        };

        file.write(&[0; std::mem::size_of::<FileSystem>()]).unwrap();
        file.seek(std::io::SeekFrom::Start(0)).unwrap();
        if let Err(e) = self.write_be(&mut file) {
            return Err(FSError::BinRw(e));
        };
        let _ = file.seek(std::io::SeekFrom::Start(0 as u64));
        self.super_block.write_be(&mut file)?;
        Ok(())
    }
}

fn main() {
    // let mut buffer = String::new();
    // let fs_controller = FSController::new();
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: [CREATE|USE] {{file_name}}");
    }
    let _command = args[1].clone();
    let fname = args[2].clone();

    println!("We are using {fname} as our disk");

    let mut fs = FileSystem::new();
    fs.instantiate_disk(&fname).unwrap();
    match fs.sync(&fname) {
        Err(FSError::Simple(s)) => eprintln!("{s}"),
        Err(FSError::BinRw(b)) => eprintln!("{b}"),
        Ok(_) => {}
    }

    let new_fs = FileSystem::open(&fname);
    assert_eq!(new_fs.super_block.magic_number, 0xDEADBEEF, "EEEEE");
    println!("{:?}", new_fs);

    // loop {
    //     buffer.clear();
    //     if let Err(e) = io::stdin().read_line(&mut buffer) {
    //         eprintln!("Error reading {e}");
    //         return;
    //     }
    // }
}
// TODO: add tests
// TODO: start using FSController instead of everything in main
