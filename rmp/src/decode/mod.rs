//! Provides various functions and structs for MessagePack decoding.
//!
//! Most of the function defined in this module will silently handle interruption error (EINTR)
//! received from the given `Read` to be in consistent state with the `Write::write_all` method in
//! the standard library.
//!
//! Any other error would immediately interrupt the parsing process. If your reader can results in
//! I/O error and simultaneously be a recoverable state (for example, when reading from
//! non-blocking socket and it returns EWOULDBLOCK) be sure that you buffer the data externally
//! to avoid data loss (using `BufRead` readers with manual consuming or some other way).

mod sint;
mod uint;

pub use self::sint::{read_nfix, read_i8, read_i16, read_i32, read_i64};
pub use self::uint::{read_pfix, read_u8, read_u16, read_u32, read_u64};

use std::error;
use std::fmt::{self, Display, Formatter};
use std::io::Read;

use byteorder::{self, ReadBytesExt};

use Marker;

/// An error that can occur when attempting to read bytes from the reader.
pub type Error = ::std::io::Error;

/// An error that can occur when attempting to read a MessagePack marker from the reader.
struct MarkerReadError(Error);

/// An error which can occur when attempting to read a MessagePack value from the reader.
#[derive(Debug)]
pub enum ValueReadError {
    /// Failed to read the marker.
    InvalidMarkerRead(Error),
    /// Failed to read the data.
    InvalidDataRead(Error),
    /// The type decoded isn't match with the expected one.
    TypeMismatch(Marker),
}

impl error::Error for ValueReadError {
    fn description(&self) -> &str {
        match *self {
            ValueReadError::InvalidMarkerRead(..) => "failed to read MessagePack marker",
            ValueReadError::InvalidDataRead(..) => "failed to read MessagePack data",
            ValueReadError::TypeMismatch(..) => {
                "the type decoded isn't match with the expected one"
            }
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            ValueReadError::InvalidMarkerRead(ref err) => Some(err),
            ValueReadError::InvalidDataRead(ref err) => Some(err),
            ValueReadError::TypeMismatch(..) => None,
        }
    }
}

impl Display for ValueReadError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        error::Error::description(self).fmt(f)
    }
}

impl From<MarkerReadError> for ValueReadError {
    fn from(err: MarkerReadError) -> ValueReadError {
        match err {
            MarkerReadError(err) => ValueReadError::InvalidMarkerRead(err),
        }
    }
}

impl From<Error> for MarkerReadError {
    fn from(err: Error) -> MarkerReadError {
        MarkerReadError(err)
    }
}

/// Attempts to read a single byte from the given reader and to decode it as a MessagePack marker.
fn read_marker<R: Read>(rd: &mut R) -> Result<Marker, MarkerReadError> {
    Ok(Marker::from_u8(try!(rd.read_u8())))
}

/// Attempts to read a single byte from the given reader and to decode it as a nil value.
///
/// According to the MessagePack specification, a nil value is represented as a single `0xc0` byte.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading the nil marker,
/// except the EINTR, which is handled internally.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
///
/// # Note
///
/// This function will silently retry on every EINTR received from the underlying `Read` until
/// successful read.
pub fn read_nil<R: Read>(rd: &mut R) -> Result<(), ValueReadError> {
    match try!(read_marker(rd)) {
        Marker::Null => Ok(()),
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read a single byte from the given reader and to decode it as a boolean value.
///
/// According to the MessagePack specification, an encoded boolean value is represented as a single
/// byte.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading the bool marker,
/// except the EINTR, which is handled internally.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
///
/// # Note
///
/// This function will silently retry on every EINTR received from the underlying `Read` until
/// successful read.
pub fn read_bool<R: Read>(rd: &mut R) -> Result<bool, ValueReadError> {
    match try!(read_marker(rd)) {
        Marker::True => Ok(true),
        Marker::False => Ok(false),
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// An error which can occur when attempting to read a MessagePack numeric value from the reader.
#[derive(Debug)]
pub enum NumValueReadError {
    /// Failed to read the marker.
    InvalidMarkerRead(Error),
    /// Failed to read the data.
    InvalidDataRead(Error),
    /// The type decoded isn't match with the expected one.
    TypeMismatch(Marker),
    /// Out of range integral type conversion attempted.
    OutOfRange,
}

impl error::Error for NumValueReadError {
    fn description(&self) -> &str {
        match *self {
            NumValueReadError::InvalidMarkerRead(..) => "failed to read MessagePack marker",
            NumValueReadError::InvalidDataRead(..) => "failed to read MessagePack data",
            NumValueReadError::TypeMismatch(..) => {
                "the type decoded isn't match with the expected one"
            }
            NumValueReadError::OutOfRange => "out of range integral type conversion attempted",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            NumValueReadError::InvalidMarkerRead(ref err) => Some(err),
            NumValueReadError::InvalidDataRead(ref err) => Some(err),
            NumValueReadError::TypeMismatch(..) => None,
            NumValueReadError::OutOfRange => None,
        }
    }
}

impl Display for NumValueReadError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        error::Error::description(self).fmt(f)
    }
}

impl From<MarkerReadError> for NumValueReadError {
    fn from(err: MarkerReadError) -> NumValueReadError {
        match err {
            MarkerReadError(err) => NumValueReadError::InvalidMarkerRead(err),
        }
    }
}

// Helper functions to map I/O error into the `InvalidDataRead` error.

fn read_data_i8<R: Read>(rd: &mut R) -> Result<i8, ValueReadError> {
    rd.read_i8().map_err(ValueReadError::InvalidDataRead)
}

fn read_data_i16<R: Read>(rd: &mut R) -> Result<i16, ValueReadError> {
    rd.read_i16::<byteorder::BigEndian>().map_err(ValueReadError::InvalidDataRead)
}

fn read_data_i32<R: Read>(rd: &mut R) -> Result<i32, ValueReadError> {
    rd.read_i32::<byteorder::BigEndian>().map_err(ValueReadError::InvalidDataRead)
}

fn read_data_i64<R: Read>(rd: &mut R) -> Result<i64, ValueReadError> {
    rd.read_i64::<byteorder::BigEndian>().map_err(ValueReadError::InvalidDataRead)
}

fn read_data_u8<R: Read>(rd: &mut R) -> Result<u8, ValueReadError> {
    rd.read_u8().map_err(ValueReadError::InvalidDataRead)
}

fn read_data_u16<R: Read>(rd: &mut R) -> Result<u16, ValueReadError> {
    rd.read_u16::<byteorder::BigEndian>().map_err(ValueReadError::InvalidDataRead)
}

fn read_data_u32<R: Read>(rd: &mut R) -> Result<u32, ValueReadError> {
    rd.read_u32::<byteorder::BigEndian>().map_err(ValueReadError::InvalidDataRead)
}

fn read_data_u64<R: Read>(rd: &mut R) -> Result<u64, ValueReadError> {
    rd.read_u64::<byteorder::BigEndian>().map_err(ValueReadError::InvalidDataRead)
}
