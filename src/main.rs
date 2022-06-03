use clap::{Arg, App};
use std::{fs, io};
use std::io::prelude::*;
use std::path::{Path,PathBuf};
use std::ffi::OsStr;
use goblin::{
    elf::section_header::SHF_COMPRESSED,
    elf::compression_header::CompressionHeader,
};
use goblin::container::Ctx;
use flate2::read::ZlibDecoder;
use anyhow::{Result, anyhow};

const DEBUG_SECTIONS: [&str; 5] = [
    ".debug_abbrev",
    ".debug_info",
    ".debug_loc",
    ".debug_line",
    ".debug_str"
];

fn build_path(output_dir: &Path, file: &Path, suffix: &str) -> PathBuf {
    // filename should always exist
    let mut file_name = file.file_name().unwrap().to_os_string();
    file_name.push(OsStr::new(suffix));
    let final_path = output_dir.join(file_name);
    return final_path;
}

fn extract_from_file(file: &Path, output_dir: &Path) -> Result<()> {
    let buffer = fs::read(file)?;
    match goblin::Object::parse(&buffer)? {
        goblin::Object::Elf(binary) => {
            let sh_strtab = binary.shdr_strtab;
            for section in binary.section_headers {
                let name_idx = section.sh_name;
                let section_name = match sh_strtab.get(name_idx) {
                    Some(n) => {
                        match n {
                            Ok(name) => name,
                            Err(_) => continue,
                        }
                    },
                    None => continue,
                };
                if DEBUG_SECTIONS.iter().any(|e| e == &section_name) {
                    let output_path = build_path(output_dir, file, section_name);
                    
                    let mut output_file = fs::File::create(output_path)?;
                    let section_start = section.sh_offset as usize;
                    let section_end = section_start + section.sh_size as usize;
                    match buffer.get(section_start..section_end) {
                        Some(buf) => {
                            if (section.sh_flags & SHF_COMPRESSED as u64) != 0 {
                                let ctx = match binary.header.container() {
                                    Ok(ctr) => {
                                        Ctx{
                                            container: ctr,
                                            le: if ctr.is_big() {scroll::Endian::Big} else {scroll::Endian::Little},
                                        }
                                    },
                                    Err(_) => continue,
                                };
                                let compression_header_size = CompressionHeader::size(ctx);
                                let mut z = ZlibDecoder::new(&buf[compression_header_size..]);
                                io::copy(&mut z, &mut output_file).unwrap();
                            } else {
                                output_file.write_all(buf)?;
                            }
                        },
                        None => {
                            eprintln!("Section {} in {} is out of range", section_name, file.to_str().unwrap_or_default());
                        }
                    }
                    
                }
            }
        },
        _ => {
            return Err(anyhow!("{} not an elf", file.to_str().unwrap()));
        }
    }
    return Ok(());
}

fn main() -> io::Result<()> {
    let matches = App::new("dwarf-extractor")
        .about("Extracts dwarf sections")
        .arg(
            Arg::new("output_dir")
            .short('o')
            .value_name("OUTPUT_DIR")
            .takes_value(true)
            .required(true)
        )
        .arg(
            Arg::new("files")
            .required(true)
            .min_values(1)
        )
        .get_matches();
    let output_dir = Path::new(matches.value_of("output_dir").unwrap());
    let files = matches.values_of("files").unwrap();
    fs::create_dir_all(output_dir)?;
    for file in files {
        match extract_from_file(Path::new(file), output_dir) {
            Ok(_) => {},
            Err(e) => {
                eprintln!("{}", e);
            }
        }
    }
    return Ok(());
}
