#![allow(dead_code, clippy::needless_pass_by_ref_mut)]
#![warn(clippy::unused_io_amount)]

extern crate futures;
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use std::io::{self, Read};

fn question_mark<T: io::Read + io::Write>(s: &mut T) -> io::Result<()> {
    s.write(b"test")?;
    //~^ ERROR: written amount is not handled
    let mut buf = [0u8; 4];
    s.read(&mut buf)?;
    //~^ ERROR: read amount is not handled
    Ok(())
}

fn unwrap<T: io::Read + io::Write>(s: &mut T) {
    s.write(b"test").unwrap();
    //~^ ERROR: written amount is not handled
    let mut buf = [0u8; 4];
    s.read(&mut buf).unwrap();
    //~^ ERROR: read amount is not handled
}

fn vectored<T: io::Read + io::Write>(s: &mut T) -> io::Result<()> {
    s.read_vectored(&mut [io::IoSliceMut::new(&mut [])])?;
    //~^ ERROR: read amount is not handled
    s.write_vectored(&[io::IoSlice::new(&[])])?;
    //~^ ERROR: written amount is not handled
    Ok(())
}

fn ok(file: &str) -> Option<()> {
    let mut reader = std::fs::File::open(file).ok()?;
    let mut result = [0u8; 0];
    reader.read(&mut result).ok()?;
    //~^ ERROR: read amount is not handled
    Some(())
}

#[allow(clippy::redundant_closure)]
#[allow(clippy::bind_instead_of_map)]
fn or_else(file: &str) -> io::Result<()> {
    let mut reader = std::fs::File::open(file)?;
    let mut result = [0u8; 0];
    reader.read(&mut result).or_else(|err| Err(err))?;
    //~^ ERROR: read amount is not handled
    Ok(())
}

#[derive(Debug)]
enum Error {
    Kind,
}

fn or(file: &str) -> Result<(), Error> {
    let mut reader = std::fs::File::open(file).unwrap();
    let mut result = [0u8; 0];
    reader.read(&mut result).or(Err(Error::Kind))?;
    //~^ ERROR: read amount is not handled
    Ok(())
}

fn combine_or(file: &str) -> Result<(), Error> {
    let mut reader = std::fs::File::open(file).unwrap();
    let mut result = [0u8; 0];
    reader
        //~^ ERROR: read amount is not handled
        .read(&mut result)
        .or(Err(Error::Kind))
        .or(Err(Error::Kind))
        .expect("error");
    Ok(())
}

fn is_ok_err<T: io::Read + io::Write>(s: &mut T) {
    s.write(b"ok").is_ok();
    //~^ ERROR: written amount is not handled
    s.write(b"err").is_err();
    //~^ ERROR: written amount is not handled
    let mut buf = [0u8; 0];
    s.read(&mut buf).is_ok();
    //~^ ERROR: read amount is not handled
    s.read(&mut buf).is_err();
    //~^ ERROR: read amount is not handled
}

async fn bad_async_write<W: AsyncWrite + Unpin>(w: &mut W) {
    w.write(b"hello world").await.unwrap();
    //~^ ERROR: written amount is not handled
}

async fn bad_async_read<R: AsyncRead + Unpin>(r: &mut R) {
    let mut buf = [0u8; 0];
    r.read(&mut buf[..]).await.unwrap();
    //~^ ERROR: read amount is not handled
}

async fn io_not_ignored_async_write<W: AsyncWrite + Unpin>(mut w: W) {
    // Here we're forgetting to await the future, so we should get a
    // warning about _that_ (or we would, if it were enabled), but we
    // won't get one about ignoring the return value.
    w.write(b"hello world");
}

fn bad_async_write_closure<W: AsyncWrite + Unpin + 'static>(w: W) -> impl futures::Future<Output = io::Result<()>> {
    let mut w = w;
    async move {
        w.write(b"hello world").await?;
        //~^ ERROR: written amount is not handled
        Ok(())
    }
}

async fn async_read_nested_or<R: AsyncRead + Unpin>(r: &mut R, do_it: bool) -> Result<[u8; 1], Error> {
    let mut buf = [0u8; 1];
    if do_it {
        r.read(&mut buf[..]).await.or(Err(Error::Kind))?;
        //~^ ERROR: read amount is not handled
    }
    Ok(buf)
}

use tokio::io::{AsyncRead as TokioAsyncRead, AsyncReadExt as _, AsyncWrite as TokioAsyncWrite, AsyncWriteExt as _};

async fn bad_async_write_tokio<W: TokioAsyncWrite + Unpin>(w: &mut W) {
    w.write(b"hello world").await.unwrap();
    //~^ ERROR: written amount is not handled
}

async fn bad_async_read_tokio<R: TokioAsyncRead + Unpin>(r: &mut R) {
    let mut buf = [0u8; 0];
    r.read(&mut buf[..]).await.unwrap();
    //~^ ERROR: read amount is not handled
}

async fn undetected_bad_async_write<W: AsyncWrite + Unpin>(w: &mut W) {
    // It would be good to detect this case some day, but the current lint
    // doesn't handle it. (The documentation says that this lint "detects
    // only common patterns".)
    let future = w.write(b"Hello world");
    future.await.unwrap();
}

fn main() {}
