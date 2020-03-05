use crate::decode::lzbuffer;
use crate::decode::lzbuffer::LZBuffer;
use crate::decode::lzma;
use crate::decode::rangecoder;
use crate::error;
use byteorder::{BigEndian, ReadBytesExt};
use std::io;
use std::io::Read;

pub fn decode_stream<R, W>(input: &mut R, output: &mut W) -> error::Result<()>
where
    R: io::BufRead,
    W: io::Write,
{
    let accum = lzbuffer::LZAccumBuffer::from_stream(output);
    let mut decoder = lzma::new_accum(accum, 0, 0, 0, None);

    loop {
        let status = input.read_u8().or_else(|e| {
            Err(error::Error::LZMAError(format!(
                "LZMA2 expected new status: {}",
                e
            )))
        })?;

        lzma_info!("LZMA2 status: {}", status);

        if status == 0 {
            lzma_info!("LZMA2 end of input");
            break;
        } else if status == 1 {
            // uncompressed reset dict
            parse_uncompressed(&mut decoder, input, true)?;
        } else if status == 2 {
            // uncompressed no reset
            parse_uncompressed(&mut decoder, input, false)?;
        } else {
            parse_lzma(&mut decoder, input, status)?;
        }
    }

    decoder.output.finish()?;
    Ok(())
}

fn parse_lzma<'a, R, W>(
    decoder: &mut lzma::DecoderState<lzbuffer::LZAccumBuffer<'a, W>>,
    input: &mut R,
    status: u8,
) -> error::Result<()>
where
    R: io::BufRead,
    W: io::Write,
{
    if status & 0x80 == 0 {
        return Err(error::Error::LZMAError(format!(
            "LZMA2 invalid status {}, must be 0, 1, 2 or >= 128",
            status
        )));
    }

    let reset_dict: bool;
    let reset_state: bool;
    let reset_props: bool;
    match (status >> 5) & 0x3 {
        0 => {
            reset_dict = false;
            reset_state = false;
            reset_props = false;
        }
        1 | 2 => {
            reset_dict = false;
            reset_state = true;
            reset_props = false;
        }
        3 => {
            reset_dict = true;
            reset_state = true;
            reset_props = true;
        }
        _ => unreachable!(),
    }

    let unpacked_size = input.read_u16::<BigEndian>().or_else(|e| {
        Err(error::Error::LZMAError(format!(
            "LZMA2 expected unpacked size: {}",
            e
        )))
    })?;
    let unpacked_size = ((((status & 0x1F) as u64) << 16) | (unpacked_size as u64)) + 1;

    let packed_size = input.read_u16::<BigEndian>().or_else(|e| {
        Err(error::Error::LZMAError(format!(
            "LZMA2 expected packed size: {}",
            e
        )))
    })?;
    let packed_size = (packed_size as u64) + 1;

    lzma_info!(
        "LZMA2 compressed block {{ unpacked_size: {}, packed_size: {}, reset_dict: {}, reset_state: {}, reset_props: {} }}",
        unpacked_size,
        packed_size,
        reset_dict,
        reset_state,
        reset_props
    );

    if reset_dict {
        decoder.output.reset()?;
    }

    if reset_state {
        let lc: u32;
        let lp: u32;
        let mut pb: u32;

        if reset_props {
            let props = input.read_u8().or_else(|e| {
                Err(error::Error::LZMAError(format!(
                    "LZMA2 expected new properties: {}",
                    e
                )))
            })?;

            pb = props as u32;
            if pb >= 225 {
                return Err(error::Error::LZMAError(format!(
                    "LZMA2 invalid properties: {} must be < 225",
                    pb
                )));
            }

            lc = pb % 9;
            pb /= 9;
            lp = pb % 5;
            pb /= 5;

            if lc + lp > 4 {
                return Err(error::Error::LZMAError(format!(
                    "LZMA2 invalid properties: lc + lp ({} + {}) must be <= 4",
                    lc, lp
                )));
            }

            lzma_info!("Properties {{ lc: {}, lp: {}, pb: {} }}", lc, lp, pb);
        } else {
            lc = decoder.lc;
            lp = decoder.lp;
            pb = decoder.pb;
        }

        decoder.reset_state(lc, lp, pb);
    }

    decoder.set_unpacked_size(Some(unpacked_size));

    let mut taken = input.take(packed_size);
    let mut rangecoder = rangecoder::RangeDecoder::new(&mut taken).or_else(|e| {
        Err(error::Error::LZMAError(format!(
            "LZMA input too short: {}",
            e
        )))
    })?;
    decoder.process(&mut rangecoder)
}

fn parse_uncompressed<'a, R, W>(
    decoder: &mut lzma::DecoderState<lzbuffer::LZAccumBuffer<'a, W>>,
    input: &mut R,
    reset_dict: bool,
) -> error::Result<()>
where
    R: io::BufRead,
    W: io::Write,
{
    let unpacked_size = input.read_u16::<BigEndian>().or_else(|e| {
        Err(error::Error::LZMAError(format!(
            "LZMA2 expected unpacked size: {}",
            e
        )))
    })?;
    let unpacked_size = (unpacked_size as usize) + 1;

    lzma_info!(
        "LZMA2 uncompressed block {{ unpacked_size: {}, reset_dict: {} }}",
        unpacked_size,
        reset_dict
    );

    if reset_dict {
        decoder.output.reset()?;
    }

    let mut buf = vec![0; unpacked_size];
    input.read_exact(buf.as_mut_slice()).or_else(|e| {
        Err(error::Error::LZMAError(format!(
            "LZMA2 expected {} uncompressed bytes: {}",
            unpacked_size, e
        )))
    })?;
    decoder.output.append_bytes(buf.as_slice());

    Ok(())
}
