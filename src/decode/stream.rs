use crate::decode::lzbuffer::{LZBuffer, LZCircularBuffer};
use crate::decode::lzma::{
    new_circular, new_circular_with_memlimit, DecoderState, LZMAParams, MAX_HEADER_LEN, START_BYTES,
};
use crate::decode::rangecoder::RangeDecoder;
use crate::decompress::Options;
use crate::error::Error;
use std::io::{BufRead, Cursor, Read, Write};

/// Maximum number of bytes to buffer while reading the header.
const MAX_TMP_LEN: usize = MAX_HEADER_LEN + START_BYTES;

/// Internal state of this streaming decoder. This is needed because we have to
/// initialize the stream before processing any data.
#[derive(Debug)]
enum State<W>
where
    W: Write,
{
    /// Stream is initialized but header values have not yet been read.
    Init(W),
    /// Header values have been read and the stream is ready to process more data.
    Run(RunState<W>),
}

/// Error type used to return the currently owned state along with the error on failure.
struct StreamStateError<W>
where
    W: Write,
{
    /// State that was owned while the error occured
    state: Option<State<W>>,
    /// IO error that occured in that state
    error: Error,
}

impl<W> StreamStateError<W>
where
    W: Write,
{
    /// Creates a new StreamStateError
    fn new(state: Option<State<W>>, error: Error) -> Self {
        Self { state, error }
    }
}

/// Structures needed while decoding data.
struct RunState<W>
where
    W: Write,
{
    decoder: DecoderState<W, LZCircularBuffer<W>>,
    range: u32,
    code: u32,
}

impl<W> RunState<W>
where
    W: Write,
{
    fn new(decoder: DecoderState<W, LZCircularBuffer<W>>, range: u32, code: u32) -> Self {
        Self {
            decoder,
            range,
            code,
        }
    }
}

impl<W> std::fmt::Debug for RunState<W>
where
    W: Write,
{
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("RunState")
            .field("range", &self.range)
            .field("code", &self.code)
            .finish()
    }
}

/// Lzma decompressor that can process multiple chunks of data using the
/// `std::io::Write` interface.
pub struct Stream<W>
where
    W: Write,
{
    /// Temporary buffer to hold data while the header is being read.
    tmp: [u8; MAX_TMP_LEN],
    /// How many bytes of the temp buffer are in use.
    tmp_len: usize,
    /// Whether the stream is initialized and ready to process data.
    /// An `Option` is used to avoid interior mutability when updating the state.
    state: Option<State<W>>,
    /// Options given when a stream is created.
    options: Options,
}

impl<W> Stream<W>
where
    W: Write,
{
    /// Initialize the stream. This will consume the `output` which is the sink
    /// implementing `std::io::Write` that will receive decompressed bytes.
    pub fn new(output: W) -> Self {
        Self::new_with_options(Options::default(), output)
    }

    /// Initialize the stream with the given `options`. This will consume the
    /// `output` which is the sink implementing `std::io::Write` that will
    /// receive decompressed bytes.
    pub fn new_with_options(options: Options, output: W) -> Self {
        Self {
            tmp: [0; MAX_TMP_LEN],
            tmp_len: 0,
            state: Some(State::Init(output)),
            options,
        }
    }

    /// Get a reference to the output sink
    pub fn get_ref(&self) -> Option<&W> {
        match &self.state {
            Some(State::Init(output)) => Some(&output),
            Some(State::Run(state)) => Some(state.decoder.output.get_output()),
            None => None,
        }
    }

    /// Get a mutable reference to the output sink
    pub fn get_mut(&mut self) -> Option<&mut W> {
        match &mut self.state {
            Some(State::Init(output)) => Some(output),
            Some(State::Run(state)) => Some(state.decoder.output.get_output_mut()),
            None => None,
        }
    }

    /// Consumes the stream and returns the output sink. This also makes sure
    /// we have properly reached the end of the stream.
    pub fn finish(mut self) -> crate::error::Result<W> {
        if let Some(state) = self.state.take() {
            match state {
                State::Init(output) => {
                    if self.tmp_len > 0 {
                        Err(Error::LZMAError("failed to read header".to_string()))
                    } else {
                        Ok(output)
                    }
                }
                State::Run(mut state) => {
                    if !self.options.allow_incomplete {
                        // Process one last time with empty input to force end of
                        // stream checks
                        let mut stream = Cursor::new(&self.tmp[0..self.tmp_len]);
                        let mut range_decoder =
                            RangeDecoder::from_parts(&mut stream, state.range, state.code);
                        state.decoder.process(&mut range_decoder)?;
                    }
                    let output = state.decoder.output.finish()?;
                    Ok(output)
                }
            }
        } else {
            // this will occur if a call to `write()` fails
            Err(Error::LZMAError(
                "can't finish stream because of previous write error".to_string(),
            ))
        }
    }

    /// Attempts to read the header and transition into a running state.
    ///
    /// This function will consume the state, returning the next state on both
    /// error and success.
    fn read_header<R: BufRead>(
        output: W,
        mut input: &mut R,
        options: &Options,
    ) -> Result<State<W>, StreamStateError<W>> {
        let params = match LZMAParams::read_header(&mut input, options) {
            Ok(params) => params,
            Err(e) => {
                return Err(StreamStateError::new(Some(State::Init(output)), e));
            }
        };

        let len = match input.fill_buf() {
            Ok(val) => val,
            Err(_) => {
                return Err(StreamStateError::new(
                    Some(State::Init(output)),
                    Error::LZMAError("need more input".to_string()),
                ));
            }
        }
        .len();

        if len < START_BYTES {
            return Err(StreamStateError::new(
                Some(State::Init(output)),
                Error::LZMAError("need more input".to_string()),
            ));
        };

        let decoder = if let Some(memlimit) = options.memlimit {
            new_circular_with_memlimit(output, params, memlimit)
        } else {
            new_circular(output, params)
        }
        .map_err(|e| StreamStateError::new(None, e))?;

        // The RangeDecoder is only kept temporarily as we are processing
        // chunks of data.
        let range_decoder =
            RangeDecoder::new(&mut input).map_err(|e| StreamStateError::new(None, e.into()))?;
        let (range, code) = range_decoder.into_parts();

        Ok(State::Run(RunState::new(decoder, range, code)))
    }

    /// Process compressed data
    fn read_data<R: BufRead>(
        mut state: RunState<W>,
        mut input: &mut R,
    ) -> std::io::Result<RunState<W>> {
        // Construct our RangeDecoder from the previous range and code
        // values.
        let mut range_decoder = RangeDecoder::from_parts(&mut input, state.range, state.code);

        // Try to process all bytes of data.
        state
            .decoder
            .process_stream(&mut range_decoder)
            .map_err(|e| -> std::io::Error { e.into() })?;

        // Save the range and code for the next chunk of data.
        let (range, code) = range_decoder.into_parts();
        state.range = range;
        state.code = code;

        Ok(RunState::new(state.decoder, range, code))
    }
}

impl<W> std::fmt::Debug for Stream<W>
where
    W: Write + std::fmt::Debug,
{
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Stream")
            .field("tmp_len", &self.tmp_len)
            .field("state", &self.state)
            .field("options", &self.options)
            .finish()
    }
}

impl<W> Write for Stream<W>
where
    W: Write,
{
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let mut input = Cursor::new(data);

        if let Some(state) = self.state.take() {
            let state = match state {
                // Read the header values and transition into a running state.
                State::Init(state) => {
                    let res = if self.tmp_len > 0 {
                        // attempt to fill the tmp buffer
                        self.tmp_len += input.read(&mut self.tmp[self.tmp_len..])?;

                        // attempt to read the header from our tmp buffer
                        let (position, res) = {
                            let mut tmp_input = Cursor::new(&self.tmp[0..self.tmp_len]);
                            let res = Stream::read_header(state, &mut tmp_input, &self.options);
                            (tmp_input.position() as usize, res)
                        };

                        // discard all bytes up to position if reading the header
                        // was successful
                        if res.is_ok() {
                            let tmp = self.tmp;
                            let new_len = self.tmp_len - position;
                            (&mut self.tmp[0..new_len])
                                .copy_from_slice(&tmp[position..self.tmp_len]);
                            self.tmp_len = new_len;
                        }
                        res
                    } else {
                        Stream::read_header(state, &mut input, &self.options)
                    };

                    match res {
                        Ok(state) => state,
                        Err(error) => {
                            // occurs when not enough input bytes were provided to
                            // read the entire header
                            if let Some(state) = error.state {
                                if self.tmp_len == 0 {
                                    // reset the cursor because we may have partial reads
                                    input.set_position(0);
                                    self.tmp_len = input.read(&mut self.tmp)?;
                                }
                                state
                            } else {
                                // occurs when the output was consumed due to a
                                // non-recoverable error
                                return Err(match error.error {
                                    Error::IOError(e) => e,
                                    Error::LZMAError(e) | Error::XZError(e) => {
                                        std::io::Error::new(std::io::ErrorKind::Other, e)
                                    }
                                });
                            }
                        }
                    }
                }

                // Process another chunk of data.
                State::Run(state) => {
                    let state = if self.tmp_len > 0 {
                        let mut tmp_input = Cursor::new(&self.tmp[0..self.tmp_len]);
                        let res = Stream::read_data(state, &mut tmp_input)?;
                        self.tmp_len = 0;
                        res
                    } else {
                        state
                    };
                    State::Run(Stream::read_data(state, &mut input)?)
                }
            };
            self.state.replace(state);
        }
        Ok(input.position() as usize)
    }

    /// Flushes the output sink. The internal buffer isn't flushed to avoid
    /// corrupting the internal state. Instead, call `finish()` to finalize the
    /// stream and flush all remaining internal data.
    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(ref mut state) = self.state {
            match state {
                State::Init(_) => Ok(()),
                State::Run(state) => state.decoder.output.get_output_mut().flush(),
            }
        } else {
            Ok(())
        }
    }
}

impl std::convert::Into<std::io::Error> for Error {
    fn into(self) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", self))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Test an empty stream
    #[test]
    fn test_stream_noop() {
        let stream = Stream::new(Vec::new());
        assert!(stream.get_ref().unwrap().is_empty());

        let output = stream.finish().unwrap();
        assert!(output.is_empty());
    }

    /// Test writing an empty slice
    #[test]
    fn test_stream_zero() {
        let mut stream = Stream::new(Vec::new());

        stream.write_all(&[]).unwrap();
        stream.write_all(&[]).unwrap();

        let output = stream.finish().unwrap();

        assert!(output.is_empty());
    }

    /// Test processing only partial data
    #[test]
    fn test_stream_incomplete() {
        let input = b"\x5d\x00\x00\x80\x00\xff\xff\xff\xff\xff\xff\xff\xff\x00\x83\xff\
                         \xfb\xff\xff\xc0\x00\x00\x00";
        // Process until this index is reached.
        let mut end = 1;

        // Test when we fail to provide the minimum number of bytes required to
        // read the header. Header size is 13 bytes but we also read the first 5
        // bytes of data.
        while end < MAX_HEADER_LEN + START_BYTES {
            let mut stream = Stream::new(Vec::new());
            stream.write_all(&input[..end]).unwrap();
            assert_eq!(stream.tmp_len, end);

            let err = stream.finish().unwrap_err();
            assert!(
                err.to_string().contains("failed to read header"),
                "error was: {}",
                err
            );

            end += 1;
        }

        // Test when we fail to provide enough bytes to terminate the stream. A
        // properly terminated stream will have a code value of 0.
        while end < input.len() {
            let mut stream = Stream::new(Vec::new());
            stream.write_all(&input[..end]).unwrap();

            // Header bytes will be buffered until there are enough to read
            if end < MAX_HEADER_LEN + START_BYTES {
                assert_eq!(stream.tmp_len, end);
            }

            let err = stream.finish().unwrap_err();
            assert!(err.to_string().contains("failed to fill whole buffer"));

            end += 1;
        }
    }

    /// Test processing all chunk sizes
    #[test]
    fn test_stream_chunked() {
        let small_input = include_bytes!("../../tests/files/small.txt");

        let mut reader = std::io::Cursor::new(&small_input[..]);
        let mut small_input_compressed = Vec::new();
        crate::lzma_compress(&mut reader, &mut small_input_compressed).unwrap();

        let input : Vec<(&[u8], &[u8])> = vec![
            (b"\x5d\x00\x00\x80\x00\xff\xff\xff\xff\xff\xff\xff\xff\x00\x83\xff\xfb\xff\xff\xc0\x00\x00\x00", b""),
            (&small_input_compressed[..], small_input)];
        for (input, expected) in input {
            for chunk in 1..input.len() {
                let mut consumed = 0;
                let mut stream = Stream::new(Vec::new());
                while consumed < input.len() {
                    let end = std::cmp::min(consumed + chunk, input.len());
                    stream.write_all(&input[consumed..end]).unwrap();
                    consumed = end;
                }
                let output = stream.finish().unwrap();
                assert_eq!(expected, &output[..]);
            }
        }
    }

    #[test]
    fn test_stream_corrupted() {
        let mut stream = Stream::new(Vec::new());
        let err = stream
            .write_all(b"corrupted bytes here corrupted bytes here")
            .unwrap_err();
        assert!(err.to_string().contains("beyond output size"));
        let err = stream.finish().unwrap_err();
        assert!(err
            .to_string()
            .contains("can\'t finish stream because of previous write error"));
    }

    #[test]
    fn test_allow_incomplete() {
        let input = include_bytes!("../../tests/files/small.txt");

        let mut reader = std::io::Cursor::new(&input[..]);
        let mut compressed = Vec::new();
        crate::lzma_compress(&mut reader, &mut compressed).unwrap();
        let compressed = &compressed[..compressed.len() / 2];

        // Should fail to finish() without the allow_incomplete option.
        let mut stream = Stream::new(Vec::new());
        stream.write_all(&compressed[..]).unwrap();
        stream.finish().unwrap_err();

        // Should succeed with the allow_incomplete option.
        let mut stream = Stream::new_with_options(
            Options {
                allow_incomplete: true,
                ..Default::default()
            },
            Vec::new(),
        );
        stream.write_all(&compressed[..]).unwrap();
        let output = stream.finish().unwrap();
        assert_eq!(output, &input[..26]);
    }
}
