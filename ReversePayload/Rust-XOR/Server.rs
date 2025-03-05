use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

const LISTEN_PORT: u16 = 9999;
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

fn handle_client(stream: TcpStream) -> io::Result<()> {
    let mut stream = XorStream::new(stream, XOR_KEY);
    let mut stream_clone = XorStream::new(stream.inner.try_clone()?, XOR_KEY);
    stream_clone.position = stream.position;

    let input_handle = thread::spawn(move || {
        let mut buffer = [0; 4096];
        loop {
            match stream.read_xor(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    print!("{}", String::from_utf8_lossy(&buffer[..n]));
                    io::stdout().flush().unwrap();
                }
                Err(e) => {
                    eprintln!("Read error: {}", e);
                    break;
                }
            }
        }
    });

    let output_handle = thread::spawn(move || {
        let mut input = String::new();
        loop {
            input.clear();
            io::stdin().read_line(&mut input).unwrap();
            if let Err(e) = stream_clone.write_xor(input.as_bytes()) {
                eprintln!("Write error: {}", e);
                break;
            }
        }
    });

    input_handle.join().unwrap();
    output_handle.join().unwrap();
    Ok(())
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", LISTEN_PORT))?;
    println!("Server listening on port {}", LISTEN_PORT);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("New connection: {}", stream.peer_addr()?);
                thread::spawn(|| {
                    handle_client(stream).unwrap_or_else(|e| eprintln!("Error: {}", e));
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }
    Ok(())
}