use std::io;
use decode::lzma2;
use decode::util;
use error;
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use crc::{crc32, crc64, Hasher32};
use std::hash::Hasher;

const XZ_MAGIC: &[u8] = &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00];
const XZ_MAGIC_FOOTER: &[u8] = &[0x59, 0x5A];

#[derive(Debug)]
enum CheckMethod {
    None,
    CRC32,
    CRC64,
    SHA256,
}

fn get_check_method(m: u16) -> error::Result<CheckMethod> {
    match m {
        0x00 => Ok(CheckMethod::None),
        0x01 => Ok(CheckMethod::CRC32),
        0x04 => Ok(CheckMethod::CRC64),
        0x0A => Ok(CheckMethod::SHA256),
        _ => Err(error::Error::XZError(format!(
            "Invalid check method {}, expected one of [0x00, 0x01, 0x04, 0x0A]",
            m
        ))),
    }
}

#[derive(Debug)]
struct Record {
    unpadded_size: u64,
    unpacked_size: u64,
}

pub fn decode_stream<R, W>(input: &mut R, output: &mut W) -> error::Result<()>
where
    R: io::BufRead,
    W: io::Write,
{
    if !util::read_tag(input, XZ_MAGIC)? {
        return Err(error::Error::XZError(
            format!("Invalid magic, expected {:?}", XZ_MAGIC),
        ));
    }

    let mut digest = crc32::Digest::new(crc32::IEEE);
    let flags = {
        let mut digested = util::HasherRead::new(input, &mut digest);
        digested.read_u16::<BigEndian>()?
    };
    let check_method = get_check_method(flags)?;
    info!("XZ check method: {:?}", check_method);

    let digest_crc32 = digest.sum32();

    let crc32 = input.read_u32::<LittleEndian>()?;
    if crc32 != digest_crc32 {
        return Err(error::Error::XZError(format!(
            "Invalid index CRC32: expected 0x{:08x} but got 0x{:08x}",
            crc32,
            digest_crc32
        )));
    }

    let mut records: Vec<Record> = vec![];
    let index_size = loop {
        let mut count_input = util::CountBufRead::new(input);
        let header_size = count_input.read_u8()?;
        info!("XZ block header_size byte: 0x{:02x}", header_size);

        if header_size == 0 {
            info!("XZ records: {:?}", records);
            check_index(&mut count_input, &records)?;
            let index_size = count_input.count();
            break index_size;
        }

        read_block(
            &mut count_input,
            output,
            &check_method,
            &mut records,
            header_size,
        )?;
    };

    let crc32 = input.read_u32::<LittleEndian>()?;
    let mut digest = crc32::Digest::new(crc32::IEEE);

    {
        let mut digested = util::HasherRead::new(input, &mut digest);

        let backward_size = digested.read_u32::<LittleEndian>()?;
        if index_size as u32 != (backward_size + 1) << 2 {
            return Err(error::Error::XZError(format!(
                "Invalid index size: expected {} but got {}",
                (backward_size + 1) << 2,
                index_size
            )));
        }

        let footer_flags = digested.read_u16::<BigEndian>()?;
        if flags != footer_flags {
            return Err(error::Error::XZError(format!(
                "Flags in header (0x{:04x}) does not match footer (0x{:04x})",
                flags,
                footer_flags
            )));
        }
    }

    let digest_crc32 = digest.sum32();
    if crc32 != digest_crc32 {
        return Err(error::Error::XZError(format!(
            "Invalid footer CRC32: expected 0x{:08x} but got 0x{:08x}",
            crc32,
            digest_crc32
        )));
    }

    if !util::read_tag(input, XZ_MAGIC_FOOTER)? {
        return Err(error::Error::XZError(format!(
            "Invalid footer magic, expected {:?}",
            XZ_MAGIC_FOOTER
        )));
    }

    if !util::is_eof(input)? {
        return Err(error::Error::XZError(
            format!("Unexpected data after last XZ block"),
        ));
    }
    Ok(())
}

fn check_index<'a, R>(
    count_input: &mut util::CountBufRead<'a, R>,
    records: &Vec<Record>,
) -> error::Result<()>
where
    R: io::BufRead,
{
    let mut digest = crc32::Digest::new(crc32::IEEE);
    let index_tag = 0u8;
    digest.write_u8(index_tag);

    {
        let mut digested = util::HasherRead::new(count_input, &mut digest);

        let num_records = get_multibyte(&mut digested)?;
        if num_records != records.len() as u64 {
            return Err(error::Error::XZError(format!(
                "Expected {} records but got {} records",
                num_records,
                records.len()
            )));
        }

        for (i, record) in records.iter().enumerate() {
            info!("XZ index checking record {}: {:?}", i, record);

            let unpadded_size = get_multibyte(&mut digested)?;
            if unpadded_size != record.unpadded_size as u64 {
                return Err(error::Error::XZError(format!(
                    "Invalid index for record {}: unpadded size ({}) does not match index ({})",
                    i,
                    record.unpadded_size,
                    unpadded_size
                )));
            }

            let unpacked_size = get_multibyte(&mut digested)?;
            if unpacked_size != record.unpacked_size as u64 {
                return Err(error::Error::XZError(format!(
                    "Invalid index for record {}: unpacked size ({}) does not match index ({})",
                    i,
                    record.unpacked_size,
                    unpacked_size
                )));
            }
        }
    }

    // TODO: create padding parser function
    let count = count_input.count();
    let padding_size = ((count ^ 0x03) + 1) & 0x03;
    info!(
        "XZ index: {} byte(s) read, {} byte(s) of padding",
        count,
        padding_size
    );

    {
        let mut digested = util::HasherRead::new(count_input, &mut digest);
        for _ in 0..padding_size {
            let byte = digested.read_u8()?;
            if byte != 0 {
                return Err(error::Error::XZError(
                    format!("Invalid index padding, must be null bytes"),
                ));
            }
        }
    }

    let digest_crc32 = digest.sum32();
    info!("XZ index checking digest 0x{:08x}", digest_crc32);

    let crc32 = count_input.read_u32::<LittleEndian>()?;
    if crc32 != digest_crc32 {
        return Err(error::Error::XZError(format!(
            "Invalid index CRC32: expected 0x{:08x} but got 0x{:08x}",
            crc32,
            digest_crc32
        )));
    }

    Ok(())
}

#[derive(Debug)]
enum FilterID {
    LZMA2,
}

fn get_filter_id(id: u64) -> error::Result<FilterID> {
    match id {
        0x21 => Ok(FilterID::LZMA2),
        _ => Err(error::Error::XZError(format!("Unknown filter id {}", id))),
    }
}

struct Filter {
    filter_id: FilterID,
    props: Vec<u8>,
}

struct BlockHeader {
    filters: Vec<Filter>,
    packed_size: Option<u64>,
    unpacked_size: Option<u64>,
}

fn read_block<'a, R, W>(
    count_input: &mut util::CountBufRead<'a, R>,
    output: &mut W,
    check_method: &CheckMethod,
    records: &mut Vec<Record>,
    header_size: u8,
) -> error::Result<bool>
where
    R: io::BufRead,
    W: io::Write,
{
    let mut digest = crc32::Digest::new(crc32::IEEE);
    digest.write_u8(header_size);
    let header_size = ((header_size as u64) << 2) - 1;

    let block_header = {
        let mut subbuf = util::SubBufRead::new(count_input, header_size as usize);
        let mut digested = io::BufReader::new(util::HasherRead::new(&mut subbuf, &mut digest));
        read_block_header(&mut digested, header_size)?
    };

    let crc32 = count_input.read_u32::<LittleEndian>()?;
    let digest_crc32 = digest.sum32();
    if crc32 != digest_crc32 {
        return Err(error::Error::XZError(format!(
            "Invalid header CRC32: expected 0x{:08x} but got 0x{:08x}",
            crc32,
            digest_crc32
        )));
    }

    let mut tmpbuf: Vec<u8> = Vec::new();
    let filters = block_header.filters;
    for (i, filter) in filters.iter().enumerate() {
        if i == 0 {
            // TODO: use SubBufRead on input if packed_size is known?
            let packed_size = decode_filter(count_input, &mut tmpbuf, filter)?;
            if let Some(expected_packed_size) = block_header.packed_size {
                if (packed_size as u64) != expected_packed_size {
                    return Err(error::Error::XZError(format!(
                        "Invalid compressed size: expected {} but got {}",
                        expected_packed_size,
                        packed_size
                    )));
                }
            }
        } else {
            let mut newbuf: Vec<u8> = Vec::new();
            decode_filter(
                &mut io::BufReader::new(tmpbuf.as_slice()),
                &mut newbuf,
                filter,
            )?;
            // TODO: does this move or copy?
            tmpbuf = newbuf;
        }
    }

    let unpacked_size = tmpbuf.len();
    info!("XZ block decompressed to {} byte(s)", tmpbuf.len());

    if let Some(expected_unpacked_size) = block_header.unpacked_size {
        if (unpacked_size as u64) != expected_unpacked_size {
            return Err(error::Error::XZError(format!(
                "Invalid decompressed size: expected {} but got {}",
                expected_unpacked_size,
                unpacked_size
            )));
        }
    }

    let count = count_input.count();
    let padding_size = ((count ^ 0x03) + 1) & 0x03;
    info!(
        "XZ block: {} byte(s) read, {} byte(s) of padding",
        count,
        padding_size
    );
    for _ in 0..padding_size {
        let byte = count_input.read_u8()?;
        if byte != 0 {
            return Err(error::Error::XZError(
                format!("Invalid block padding, must be null bytes"),
            ));
        }
    }
    check_checksum(count_input, tmpbuf.as_slice(), check_method)?;

    output.write_all(tmpbuf.as_slice())?;
    records.push(Record {
        unpadded_size: (count_input.count() - padding_size) as u64,
        unpacked_size: unpacked_size as u64,
    });

    let finished = false;
    Ok(finished)
}

fn check_checksum<R>(input: &mut R, buf: &[u8], check_method: &CheckMethod) -> error::Result<()>
where
    R: io::BufRead,
{
    match *check_method {
        CheckMethod::None => (),
        CheckMethod::CRC32 => {
            util::discard(input, 4)?;
            let crc32 = input.read_u32::<LittleEndian>()?;
            let digest_crc32 = crc32::checksum_ieee(buf);
            if crc32 != digest_crc32 {
                return Err(error::Error::XZError(format!(
                    "Invalid block CRC32, expected 0x{:08x} but got 0x{:08x}",
                    crc32,
                    digest_crc32
                )));
            }
        }
        CheckMethod::CRC64 => {
            let crc64 = input.read_u64::<LittleEndian>()?;
            let digest_crc64 = crc64::checksum_ecma(buf);
            if crc64 != digest_crc64 {
                return Err(error::Error::XZError(format!(
                    "Invalid block CRC64, expected 0x{:016x} but got 0x{:016x}",
                    crc64,
                    digest_crc64
                )));
            }
        }
        // TODO
        CheckMethod::SHA256 => unimplemented!(),
    }
    Ok(())
}

fn decode_filter<R, W>(input: &mut R, output: &mut W, filter: &Filter) -> error::Result<usize>
where
    R: io::BufRead,
    W: io::Write,
{
    let mut count_input = util::CountBufRead::new(input);
    match filter.filter_id {
        FilterID::LZMA2 => {
            if filter.props.len() != 1 {
                return Err(error::Error::XZError(format!(
                    "Invalid properties for filter {:?}",
                    filter.filter_id
                )));
            }
            // TODO: properties??
            lzma2::decode_stream(&mut count_input, output)?;
            Ok(count_input.count())
        }
    }
}

fn read_block_header<R>(input: &mut R, header_size: u64) -> error::Result<BlockHeader>
where
    R: io::BufRead,
{
    let flags = input.read_u8()?;
    let num_filters = (flags & 0x03) + 1;
    let reserved = flags & 0x3C;
    let has_packed_size = flags & 0x40 != 0;
    let has_unpacked_size = flags & 0x80 != 0;

    info!(
        "XZ block header: {{ header_size: {}, flags: {}, num_filters: {}, has_packed_size: {}, has_unpacked_size: {} }}",
        header_size,
        flags,
        num_filters,
        has_packed_size,
        has_unpacked_size
    );

    if reserved != 0 {
        return Err(error::Error::XZError(format!(
            "Invalid block flags {}, reserved bits (mask 0x3C) must be zero",
            flags
        )));
    }

    let packed_size = if has_packed_size {
        Some(get_multibyte(input)?)
    } else {
        None
    };

    let unpacked_size = if has_unpacked_size {
        Some(get_multibyte(input)?)
    } else {
        None
    };

    info!(
        "XZ block header: {{ packed_size: {:?}, unpacked_size: {:?} }}",
        packed_size,
        unpacked_size
    );

    let mut filters: Vec<Filter> = vec![];
    for _ in 0..num_filters {
        let filter_id = get_filter_id(get_multibyte(input)?)?;
        let size_of_properties = get_multibyte(input)?;

        info!(
            "XZ filter: {{ filter_id: {:?}, size_of_properties: {} }}",
            filter_id,
            size_of_properties
        );

        // Early abort to avoid allocating a large vector
        if size_of_properties > header_size {
            return Err(error::Error::XZError(format!(
                "Size of filter properties exceeds block header size ({} > {})",
                size_of_properties,
                header_size
            )));
        }

        let mut buf = vec![0; size_of_properties as usize];
        try!(input.read_exact(buf.as_mut_slice()).or_else(|e| {
            Err(error::Error::XZError(format!(
                "Could not read filter properties of size {}: {}",
                size_of_properties,
                e
            )))
        }));

        info!("XZ filter properties: {:?}", buf);

        filters.push(Filter {
            filter_id,
            props: buf,
        })
    }

    if !util::flush_zero_padding(input)? {
        return Err(error::Error::XZError(
            format!("Invalid block header padding, must be null bytes"),
        ));
    }

    Ok(BlockHeader {
        filters,
        packed_size,
        unpacked_size,
    })
}

pub fn get_multibyte<R>(input: &mut R) -> error::Result<u64>
where
    R: io::Read,
{
    let mut result = 0;
    for i in 0..9 {
        let byte = input.read_u8()?;
        result ^= ((byte & 0x7F) as u64) << (i * 7);
        if (byte & 0x80) == 0 {
            return Ok(result);
        }
    }

    Err(error::Error::XZError(
        format!("Invalid multi-byte encoding"),
    ))
}
